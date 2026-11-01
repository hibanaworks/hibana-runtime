use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote};
use syn::{
    Attribute, Block, Expr, FnArg, ItemFn, Pat, ReturnType, Stmt, Type, parse_macro_input,
    parse_quote,
    visit_mut::{self, VisitMut},
};

/// Drive localside declarations for the lifetime of their enclosing async body.
#[proc_macro_attribute]
pub fn scope(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(Span::call_site(), "scope takes no arguments")
            .into_compile_error()
            .into();
    }
    expand(parse_macro_input!(input as ItemFn), None)
}

/// A platform entry point. Supply a driver, or enable `host` to use the macOS
/// wake/wait driver without an argument.
#[proc_macro_attribute]
pub fn main(args: TokenStream, input: TokenStream) -> TokenStream {
    let entry = if args.is_empty() {
        Entry::Host
    } else {
        Entry::Driver(parse_macro_input!(args as Expr))
    };
    expand(parse_macro_input!(input as ItemFn), Some(entry))
}

enum Entry {
    Driver(Expr),
    Host,
}

fn expand(mut function: ItemFn, entry: Option<Entry>) -> TokenStream {
    if function.sig.asyncness.is_none() {
        return syn::Error::new_spanned(&function.sig, "a scope must be async")
            .into_compile_error()
            .into();
    }
    let output: Type = match &function.sig.output {
        ReturnType::Type(_, ty) => (**ty).clone(),
        ReturnType::Default => parse_quote!(()),
    };
    let mut lower = Lower {
        error: quote!(<#output as ::hibana_runtime::__private::Outcome>::Error),
        diagnostics: Tokens::new(),
        next: 0,
    };
    lower.body(&mut function.block);
    if !lower.diagnostics.is_empty() {
        return lower.diagnostics.into();
    }
    if let Some(entry) = entry {
        function.sig.asyncness = None;
        let body = function.block;
        function.block = Box::new(match entry {
            Entry::Driver(driver) => parse_quote!({
                ::hibana_runtime::__private::block_on(
                    ::core::pin::pin!(async move #body), #driver
                )
            }),
            Entry::Host => parse_quote!({
                ::hibana_runtime::__private::host_block_on(::core::pin::pin!(async move #body))
            }),
        });
    } else {
        // Ordinary Rust 2024 function signatures support lifetime elision in
        // paths such as Endpoint<CLIENT>. Return-position impl Future captures
        // those input lifetimes without exposing a placeholder in user code.
        // Bind patterns inside the future and retain even unused arguments,
        // preserving async fn's lazy destructuring and argument ownership.
        let bindings: Vec<_> = function
            .sig
            .inputs
            .iter_mut()
            .enumerate()
            .map(|(index, argument)| match argument {
                FnArg::Typed(argument) => {
                    let pattern = argument.pat.clone();
                    let name = match argument.pat.as_mut() {
                        syn::Pat::Ident(pattern)
                            if pattern.by_ref.is_none() && pattern.subpat.is_none() =>
                        {
                            pattern.mutability = None;
                            pattern.ident.clone()
                        }
                        _ => {
                            let name = format_ident!(
                                "__hibana_argument{index}",
                                span = Span::mixed_site()
                            );
                            *argument.pat = parse_quote!(#name);
                            name
                        }
                    };
                    let attrs = &argument.attrs;
                    quote!(
                        #(#attrs)* let _ = &#name;
                        #(#attrs)* let #pattern = #name;
                    )
                }
                FnArg::Receiver(_) => quote!(let _ = &self;),
            })
            .collect();
        function.sig.asyncness = None;
        function.sig.output = parse_quote!(-> impl ::core::future::Future<Output = #output>);
        let body = function.block;
        function.block = Box::new(parse_quote!({
            async move { #(#bindings)* #body }
        }));
    }
    quote!(#function).into()
}

struct Lower {
    error: Tokens,
    diagnostics: Tokens,
    next: usize,
}

fn array_child(statement: &Stmt) -> Option<&syn::ExprForLoop> {
    let Stmt::Expr(Expr::ForLoop(array), _) = statement else {
        return None;
    };
    matches!(
        array.body.stmts.as_slice(),
        [Stmt::Expr(Expr::Async(_), Some(_))]
    )
    .then_some(array)
}

fn declaration(statement: &Stmt) -> bool {
    matches!(statement, Stmt::Expr(Expr::Async(_), Some(_))) || array_child(statement).is_some()
}

// Awaiting an owned future variable transfers exactly that future. Retaining
// both its async capture and a second await slot would duplicate its buffer.
// Calls and other expressions stay lazy inside their async body.
fn forwarded(block: &Block) -> Option<&Expr> {
    let [Stmt::Expr(Expr::Await(awaited), None)] = block.stmts.as_slice() else {
        return None;
    };
    matches!(awaited.base.as_ref(), Expr::Path(_)).then_some(&awaited.base)
}

fn async_body(expression: &Expr) -> Option<&syn::ExprAsync> {
    match expression {
        Expr::Async(body) => Some(body),
        Expr::Paren(paren) if paren.attrs.is_empty() => async_body(&paren.expr),
        Expr::Group(group) if group.attrs.is_empty() => async_body(&group.expr),
        _ => None,
    }
}

fn binding_names(pattern: &Pat) -> Option<Vec<String>> {
    match pattern {
        Pat::Ident(binding) if binding.by_ref.is_none() && binding.subpat.is_none() => {
            Some(vec![binding.ident.to_string()])
        }
        Pat::Tuple(tuple) => tuple
            .elems
            .iter()
            .try_fold(Vec::new(), |mut names, pattern| {
                names.extend(binding_names(pattern)?);
                Some(names)
            }),
        Pat::Slice(array) => array
            .elems
            .iter()
            .try_fold(Vec::new(), |mut names, pattern| {
                names.extend(binding_names(pattern)?);
                Some(names)
            }),
        _ => None,
    }
}

fn mentions(tokens: Tokens, names: &[String]) -> bool {
    tokens.into_iter().any(|token| match token {
        proc_macro2::TokenTree::Ident(name) => names.iter().any(|input| name == input),
        proc_macro2::TokenTree::Group(group) => mentions(group.stream(), names),
        _ => false,
    })
}

fn storage(children: &[Tokens]) -> Tokens {
    match children.len() {
        0 => quote!(()),
        1 => children[0].clone(),
        count => {
            let middle = count / 2;
            let left = storage(&children[..middle]);
            let right = storage(&children[middle..]);
            quote!(::hibana_runtime::__private::Pair { left: #left, right: #right })
        }
    }
}

/// Only returns in this async body belong to its parent. Nested futures and
/// closures retain their own return boundary.
struct ParentReturns(syn::Lifetime);

impl VisitMut for ParentReturns {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if let Expr::Return(returned) = expression {
            let label = &self.0;
            let value = returned.expr.as_ref().map(|value| quote!(#value));
            *expression = parse_quote!(break #label (#value).map(::core::option::Option::Some));
        } else {
            visit_mut::visit_expr_mut(self, expression);
        }
    }

    fn visit_expr_async_mut(&mut self, _: &mut syn::ExprAsync) {}
    fn visit_expr_closure_mut(&mut self, _: &mut syn::ExprClosure) {}
    fn visit_item_mut(&mut self, _: &mut syn::Item) {}
}

struct ImmediateBody<'a> {
    label: syn::Lifetime,
    error: &'a Tokens,
}

impl VisitMut for ImmediateBody<'_> {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        let label = self.label.clone();
        match expression {
            Expr::Return(returned) => {
                let value = returned.expr.as_ref();
                *expression = parse_quote!(break #label (#value));
            }
            Expr::Try(tried) => {
                self.visit_expr_mut(&mut tried.expr);
                let value = &tried.expr;
                let error = self.error;
                *expression = parse_quote!(match (#value) {
                    ::core::result::Result::Ok(value) => value,
                    ::core::result::Result::Err(error) => break #label
                        (::core::result::Result::Err(<#error>::from(error))),
                });
            }
            _ => visit_mut::visit_expr_mut(self, expression),
        }
    }
    fn visit_expr_async_mut(&mut self, _: &mut syn::ExprAsync) {}
    fn visit_expr_closure_mut(&mut self, _: &mut syn::ExprClosure) {}
    fn visit_item_mut(&mut self, _: &mut syn::Item) {}
}

impl Lower {
    fn name(&mut self, purpose: &str) -> syn::Ident {
        let name = format_ident!(
            "__hibana_{}{}",
            purpose,
            self.next,
            span = Span::mixed_site()
        );
        self.next += 1;
        name
    }

    fn placed(&mut self, attrs: &[Attribute], future: Tokens, placements: &mut Tokens) -> Tokens {
        let name = self.name("future");
        placements.extend(quote!(
            #(#attrs)* let #name = ::core::pin::pin!(::core::option::Option::Some(#future));
        ));
        quote!({ #(#attrs)* {
            ::hibana_runtime::__private::Leaf {
                future: ::core::option::Option::Some(::hibana_runtime::__private::placed(#name)),
            }
        }})
    }

    fn child(&mut self, block: &mut Block) {
        // A trailing declaration returns its successful child values. A normal
        // unit-valued body keeps its ordinary success value ().
        let has_value = match block.stmts.last() {
            Some(statement) if declaration(statement) => true,
            Some(Stmt::Expr(Expr::ForLoop(_) | Expr::While(_), None)) => false,
            Some(Stmt::Expr(Expr::If(branch), None)) => branch.else_branch.is_some(),
            Some(Stmt::Expr(_, None)) => true,
            _ => false,
        };
        if !has_value {
            let error = &self.error;
            block.stmts.push(Stmt::Expr(
                parse_quote!(::core::result::Result::Ok::<(), #error>(())),
                None,
            ));
        }
        // A direct receive may have a narrower error type than its enclosing
        // scope. Convert the final awaited Result just as `?` would do.
        if let Some(Stmt::Expr(tail @ Expr::Await(_), None)) = block.stmts.last_mut() {
            let error = &self.error;
            *tail = parse_quote!((#tail).map_err(<#error>::from));
        }
        self.body(block);
    }

    fn body(&mut self, block: &mut Block) {
        let collects = block.stmts.last().is_some_and(declaration);
        for statement in &mut block.stmts {
            if array_child(statement).is_some() {
                let Stmt::Expr(Expr::ForLoop(array), _) = statement else {
                    unreachable!()
                };
                if array.label.is_some() {
                    self.diagnostics.extend(
                        syn::Error::new_spanned(
                            &array.label,
                            "an array of async children has no loop control-flow label",
                        )
                        .into_compile_error(),
                    );
                }
                self.visit_expr_mut(&mut array.expr);
                let Stmt::Expr(Expr::Async(child), _) = &mut array.body.stmts[0] else {
                    unreachable!()
                };
                if forwarded(&child.block).is_none() {
                    self.child(&mut child.block);
                }
            } else if let Stmt::Expr(Expr::Async(child), Some(_)) = statement {
                if forwarded(&child.block).is_none() {
                    self.child(&mut child.block);
                }
            } else {
                visit_mut::visit_stmt_mut(self, statement);
            }
        }
        let source = core::mem::take(&mut block.stmts);
        if !collects {
            block.stmts = self.statements(source, &mut [].iter(), false);
            return;
        }

        // Outputs are ordinary unpinned data, not futures, receivers or wakers.
        // Allocate one per declaration before the parent splits into async
        // continuations; construct each future only at its source position.
        let outputs: Vec<_> = source
            .iter()
            .filter(|statement| declaration(statement))
            .map(|statement| {
                let attrs = match statement {
                    Stmt::Expr(Expr::Async(child), _) => child.attrs.clone(),
                    Stmt::Expr(Expr::ForLoop(array), _) => array.attrs.clone(),
                    _ => unreachable!(),
                };
                (self.name("value"), attrs)
            })
            .collect();
        let declarations = outputs.iter().map(|(name, attrs)| {
            quote!(
                #(#attrs)* let mut #name = ::hibana_runtime::__private::empty_values();
            )
        });
        let values: Vec<_> = outputs
            .iter()
            .map(|(name, attrs)| {
                quote!(
                    #(#attrs)* ::hibana_runtime::__private::take_values(#name)
                )
            })
            .collect();
        let value = match values.as_slice() {
            [value] => quote!({ #value }),
            _ => quote!((#(#values,)*)),
        };
        let slots: Vec<_> = outputs.iter().map(|(name, _)| name.clone()).collect();
        let mut source = source;
        let error = &self.error;
        source.push(Stmt::Expr(
            parse_quote!(::core::result::Result::Ok::<_, #error>(::core::option::Option::None)),
            None,
        ));
        let mut remaining_outputs = slots.iter();
        let body = self.statements(source, &mut remaining_outputs, true);
        assert!(
            remaining_outputs.next().is_none(),
            "every child has one output slot"
        );
        let error = &self.error;
        block.stmts = vec![Stmt::Expr(
            parse_quote!({
                #(#declarations)*
                match { #(#body)* }? {
                    ::core::option::Option::Some(value) => ::core::result::Result::Ok::<_, #error>(value),
                    ::core::option::Option::None => ::core::result::Result::Ok::<_, #error>(#value),
                }
            }),
            None,
        )];
    }

    fn statements<'a>(
        &mut self,
        mut statements: Vec<Stmt>,
        outputs: &mut impl Iterator<Item = &'a syn::Ident>,
        collects: bool,
    ) -> Vec<Stmt> {
        // Every parent segment has its own real async return boundary. A
        // labelled block keeps source early returns inside that segment without
        // adding an async adapter around the inputs and copying their storage.
        let label = syn::Lifetime::new(&format!("'{}", self.name("parent")), Span::mixed_site());
        let first = statements.iter().position(declaration);
        let mut remaining = first
            .map(|first| statements.split_off(first))
            .unwrap_or_default();
        if collects {
            let mut returns = ParentReturns(label.clone());
            for statement in &mut statements {
                returns.visit_stmt_mut(statement);
            }
        }
        let Some(_) = first else {
            return if collects {
                vec![Stmt::Expr(parse_quote!(#label: { #(#statements)* }), None)]
            } else {
                statements
            };
        };
        let count = remaining
            .iter()
            .take_while(|statement| declaration(statement))
            .count();
        let suffix = remaining.split_off(count);
        // When a binding only supplies owned futures to these children, end
        // its temporary storage inside the final pinned rows' initializer.
        // Other locals, borrowed inputs and parent effects keep their source
        // scope. In particular, never move an owned buffer behind its borrower.
        let fused = if collects
            && let Some(Stmt::Local(binding)) = statements.last()
            && binding.attrs.is_empty()
            && let Some(mut names) = binding_names(&binding.pat)
        {
            let inputs: Option<Vec<_>> = remaining
                .iter()
                .map(|statement| {
                    let Stmt::Expr(Expr::Async(child), _) = statement else {
                        return None;
                    };
                    if !child.attrs.is_empty() {
                        return None;
                    }
                    let Expr::Path(path) = forwarded(&child.block)? else {
                        return None;
                    };
                    path.path.get_ident().map(ToString::to_string)
                })
                .collect();
            names.sort();
            if let Some(mut inputs) = inputs {
                inputs.sort();
                if names == inputs && !mentions(quote!(#(#suffix)*), &names) {
                    statements.pop()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        let mut placements = Tokens::new();
        let mut children = Vec::new();
        let error = self.error.clone();
        for statement in remaining {
            let (attrs, future, array) = match statement {
                Stmt::Expr(Expr::Async(child), _) => {
                    let attrs = child.attrs;
                    let capture = child.capture;
                    let block = child.block;
                    let future = if let Some(input) = forwarded(&block) {
                        quote!(#input)
                    } else {
                        quote!(::hibana_runtime::__private::call_once(
                            async #capture || -> ::core::result::Result<_, #error> #block
                        ))
                    };
                    (attrs, future, false)
                }
                Stmt::Expr(Expr::ForLoop(array), _) => {
                    let attrs = array.attrs;
                    let input = array.expr;
                    let pattern = array.pat;
                    let Stmt::Expr(Expr::Async(child), _) =
                        array.body.stmts.into_iter().next().expect("array child")
                    else {
                        unreachable!()
                    };
                    let block = child.block;
                    let future = if let Some(input) = forwarded(&block) {
                        quote!(#input)
                    } else {
                        quote!(
                            ::hibana_runtime::__private::call_once(
                                async move || -> ::core::result::Result<_, #error> #block
                            )
                        )
                    };
                    let future = quote!((#input).map(|#pattern| #future));
                    (attrs, future, true)
                }
                _ => unreachable!("only async and fixed-array declarations"),
            };
            if collects {
                let slot = outputs.next().expect("one output slot per child");
                if fused.is_some() {
                    children
                        .push(quote!(::hibana_runtime::__private::input_rows(#future, &mut #slot)));
                    continue;
                }
                let pinned = self.name("inputs");
                placements.extend(quote!(
                    #(#attrs)* let #pinned = ::core::pin::pin!(
                        ::hibana_runtime::__private::input_rows(#future, &mut #slot)
                    );
                ));
                children.push(quote!({ #(#attrs)* {
                    ::hibana_runtime::__private::placed_rows::<#error, _>(#pinned)
                }}));
            } else {
                let future = if array {
                    quote!(::hibana_runtime::__private::Runtime::<#error, _>::new(
                        (#future).map(|future| ::hibana_runtime::__private::Leaf {
                            future: ::core::option::Option::Some(future),
                        })
                    ))
                } else {
                    future
                };
                children.push(self.placed(&attrs, future, &mut placements));
            }
        }
        if let Some(binding) = fused {
            let pinned = self.name("inputs");
            let rows = storage(&children);
            placements.extend(quote!(let #pinned = ::core::pin::pin!({ #binding #rows });));
            children = vec![quote!(::hibana_runtime::__private::placed_rows::<#error, _>(#pinned))];
        }
        let tail: Expr = if suffix.is_empty() {
            let rows = storage(&children);
            parse_quote!({
                #placements
                ::hibana_runtime::__private::Runtime::<#error, _>::new(#rows).await
            })
        } else {
            let output = self.name("output");
            let suffix = Block {
                brace_token: Default::default(),
                stmts: self.statements(suffix, outputs, collects),
            };
            let parent = self.placed(&[], quote!(
                ::hibana_runtime::__private::store_result(
                    ::hibana_runtime::__private::call_once(async || -> ::core::result::Result<_, #error> #suffix),
                    &mut #output,
                )
            ), &mut placements);
            children.push(parent);
            let rows = storage(&children);
            parse_quote!({
                let mut #output = ::core::option::Option::None;
                {
                    #placements
                    ::hibana_runtime::__private::Runtime::<#error, _>::new(#rows).await?;
                }
                ::core::result::Result::Ok::<_, #error>(#output.expect("completed async body produced its value"))
            })
        };
        statements.push(Stmt::Expr(tail, None));
        if collects {
            vec![Stmt::Expr(parse_quote!(#label: { #(#statements)* }), None)]
        } else {
            statements
        }
    }
}

impl VisitMut for Lower {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        if let Expr::Await(awaited) = expression
            && let Some(body) = async_body(&awaited.base)
            && body.capture.is_none()
            && body.block.stmts.iter().any(declaration)
        {
            // An immediately awaited scope needs a lifetime/return boundary,
            // not another future holding copies of captured input buffers.
            let mut block = body.block.clone();
            self.body(&mut block);
            let label = syn::Lifetime::new(&format!("'{}", self.name("scope")), Span::mixed_site());
            ImmediateBody {
                label: label.clone(),
                error: &self.error,
            }
            .visit_block_mut(&mut block);
            let mut result: syn::ExprBlock = parse_quote!(#label: #block);
            result.attrs = body.attrs.iter().chain(&awaited.attrs).cloned().collect();
            *expression = Expr::Block(result);
            return;
        }
        visit_mut::visit_expr_mut(self, expression);
    }
    fn visit_block_mut(&mut self, block: &mut Block) {
        for statement in &mut block.stmts {
            if declaration(statement) {
                self.diagnostics.extend(
                    syn::Error::new_spanned(
                        statement,
                        "async children belong to an async body; ordinary blocks never wait",
                    )
                    .into_compile_error(),
                );
            } else {
                visit_mut::visit_stmt_mut(self, statement);
            }
        }
    }
    fn visit_expr_async_mut(&mut self, expr: &mut syn::ExprAsync) {
        self.body(&mut expr.block);
    }
    fn visit_expr_closure_mut(&mut self, _: &mut syn::ExprClosure) {}
    fn visit_item_mut(&mut self, _: &mut syn::Item) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_array_scopes_parse_after_lowering() {
        let mut block: Block = parse_quote!({
            let values = async {
                for input in input {
                    async move { input.await };
                }
            }
            .await?;
            let nested = async {
                for row in [[1, 2], [3, 4]]
                    .map(|row| row.map(|value| core::future::ready(Ok::<_, ()>(value))))
                {
                    async move {
                        for input in row {
                            async move { input.await };
                        }
                    };
                }
            }
            .await?;
            let empty = async {
                for input in [] as [core::future::Ready<Result<u32, ()>>; 0] {
                    async move { input.await };
                }
            }
            .await?;
            Ok((values, nested, empty))
        });
        let mut lower = Lower {
            error: quote!(()),
            diagnostics: Tokens::new(),
            next: 0,
        };
        lower.body(&mut block);
        assert!(lower.diagnostics.is_empty());
        syn::parse2::<Block>(quote!(#block)).unwrap();
    }
}
