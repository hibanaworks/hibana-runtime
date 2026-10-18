use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as Tokens};
use quote::{format_ident, quote};
use syn::{
    Attribute, Block, Expr, ItemFn, Lifetime, ReturnType, Stmt, Type,
    parse::{ParseStream, Parser},
    parse_macro_input, parse_quote,
    visit_mut::{self, VisitMut},
};

/// Declare standalone async statements as scoped, automatically driven localsides.
#[proc_macro_attribute]
pub fn localside(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(Span::call_site(), "localside takes no arguments")
            .into_compile_error()
            .into();
    }
    expand(parse_macro_input!(input as ItemFn), None)
}

/// An async entry point. Its one argument is the platform's wake/wait driver.
#[proc_macro_attribute]
pub fn main(args: TokenStream, input: TokenStream) -> TokenStream {
    let driver = parse_macro_input!(args as Expr);
    expand(parse_macro_input!(input as ItemFn), Some(driver))
}

fn expand(mut function: ItemFn, driver: Option<Expr>) -> TokenStream {
    if function.sig.asyncness.is_none() {
        return syn::Error::new_spanned(&function.sig, "a localside must be async")
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
    lower.block(&mut function.block, true);
    if !lower.diagnostics.is_empty() {
        return lower.diagnostics.into();
    }
    if let Some(driver) = driver {
        function.sig.asyncness = None;
        let body = function.block;
        function.block = Box::new(parse_quote!({
            ::hibana_runtime::__private::block_on(async move #body, #driver)
        }));
    }
    quote!(#function).into()
}

struct Lower {
    error: Tokens,
    diagnostics: Tokens,
    next: usize,
}

fn is_marker(attribute: &Attribute) -> bool {
    attribute
        .path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "localside")
}

// Read the outer attributes uniformly for calls, paths, method calls, blocks,
// and future-valued macro invocations. The input already is a parsed Rust
// statement, so this round trip cannot accept a different source language.
fn parts(statement: &Stmt) -> Option<(Vec<Attribute>, Expr)> {
    let tokens = match statement {
        Stmt::Expr(expr, Some(_)) => quote!(#expr),
        Stmt::Macro(statement) if statement.semi_token.is_some() => {
            let attrs = &statement.attrs;
            let mac = &statement.mac;
            quote!(#(#attrs)* #mac)
        }
        _ => return None,
    };
    let parser = |input: ParseStream<'_>| {
        let attrs = Attribute::parse_outer(input)?;
        let expr = input.parse::<Expr>()?;
        Ok((attrs, expr))
    };
    Some(
        parser
            .parse2(tokens)
            .expect("round trip of a parsed expression statement"),
    )
}

fn declaration(statement: &Stmt) -> bool {
    matches!(statement, Stmt::Expr(Expr::Async(_), Some(_)))
        || parts(statement).is_some_and(|(attrs, _)| attrs.iter().any(is_marker))
}

impl Lower {
    fn block(&mut self, block: &mut Block, boundary: bool) {
        for statement in &mut block.stmts {
            if let Stmt::Expr(Expr::Async(child), Some(_)) = statement {
                // Statement bodies need no repeated `Ok(())`. An explicit
                // tail value is already a Result and retains its usual type.
                let has_tail = matches!(child.block.stmts.last(), Some(Stmt::Expr(_, None)));
                if !has_tail {
                    let error = &self.error;
                    child.block.stmts.push(Stmt::Expr(
                        parse_quote!(
                            ::core::result::Result::Ok::<(), #error>(())
                        ),
                        None,
                    ));
                }
                self.block(&mut child.block, true);
            } else {
                visit_mut::visit_stmt_mut(self, statement);
            }
        }
        block.stmts = self.statements(core::mem::take(&mut block.stmts), boundary);
    }

    fn statements(&mut self, statements: Vec<Stmt>, boundary: bool) -> Vec<Stmt> {
        let Some(first) = statements.iter().position(declaration) else {
            return statements;
        };
        let mut prefix = statements;
        let mut remaining = prefix.split_off(first);
        let count = remaining.iter().take_while(|s| declaration(s)).count();
        let suffix = remaining.split_off(count);
        let declarations: Vec<_> = remaining
            .into_iter()
            .map(|stmt| {
                let (attrs, future) =
                    parts(&stmt).expect("a declaration is an expression statement");
                let attrs: Vec<_> = attrs
                    .into_iter()
                    .filter(|attribute| {
                        if !is_marker(attribute) {
                            return true;
                        }
                        if !matches!(attribute.meta, syn::Meta::Path(_)) {
                            self.diagnostics.extend(
                                syn::Error::new_spanned(
                                    attribute,
                                    "a localside declaration takes no arguments",
                                )
                                .into_compile_error(),
                            );
                        }
                        false
                    })
                    .collect();
                (attrs, future)
            })
            .collect();
        let task_name = format_ident!("__hibana_tasks{}", self.next, span = Span::mixed_site());
        self.next += 1;
        let bindings = declarations.into_iter().map(|(attrs, child)| {
            quote!(
                #(#attrs)*
                let #task_name = #task_name.localside(#child);
            )
        });
        let error = self.error.clone();
        let setup = quote!(
            let #task_name = ::hibana_runtime::__private::Runtime::<#error>::new();
            #(#bindings)*
        );

        // A scope containing only declarations needs no separate parent slot.
        if suffix.is_empty() {
            prefix.push(parse_quote!({ #setup #task_name.await?; }));
            return prefix;
        }

        let id = self.next;
        self.next += 1;
        let flow = format_ident!("__HibanaExit{id}", span = Span::mixed_site());
        let output = format_ident!("__hibana_output{id}", span = Span::mixed_site());
        let body_value = format_ident!("__hibana_body{id}", span = Span::mixed_site());
        let mut suffix = Block {
            brace_token: Default::default(),
            stmts: self.statements(suffix, boundary),
        };
        let mut exits = Exits {
            flow: flow.clone(),
            error: error.clone(),
            inner: Vec::new(),
            exits: Vec::new(),
        };
        exits.visit_block_mut(&mut suffix);
        let (definition, success, value) = if exits.exits.is_empty() {
            (quote!(), quote!(#body_value), quote!(#output))
        } else {
            let types: Vec<_> = exits
                .exits
                .iter()
                .filter_map(|exit| exit.ty.clone())
                .collect();
            let arguments: Vec<_> = types.iter().map(|_| quote!(_)).collect();
            let variants = exits.exits.iter().map(|exit| {
                let variant = &exit.variant;
                match &exit.ty {
                    Some(ty) => quote!(#variant(#ty)),
                    None => quote!(#variant),
                }
            });
            let arms = exits.exits.iter().map(|exit| {
                let variant = &exit.variant;
                let jump = &exit.jump;
                let pattern = if exit.ty.is_some() {
                    quote!((value))
                } else {
                    quote!()
                };
                quote!(::core::ops::ControlFlow::Break(#flow::#variant #pattern) => #jump,)
            });
            (
                quote!(enum #flow <#(#types),*> { #(#variants),* }),
                quote!(::core::ops::ControlFlow::<#flow <#(#arguments),*>, _>::Continue(#body_value)),
                quote!(match #output {
                    ::core::ops::ControlFlow::Continue(value) => value,
                    #(#arms)*
                }),
            )
        };
        let body_binding = if boundary {
            quote!(let #body_value: ::core::result::Result<_, #error> = #suffix;)
        } else {
            quote!(let #body_value = #suffix;)
        };
        let check_result = if boundary {
            quote!(
                #[allow(unreachable_code)]
                let #body_value = ::core::result::Result::map(
                    #body_value, ::core::result::Result::Ok::<_, #error>
                )?;
            )
        } else {
            quote!()
        };
        let lowered: Block = parse_quote!({
            #definition
            let mut #output = ::core::option::Option::None;
            {
                #setup
                #task_name.localside(async {
                    #output = ::core::option::Option::Some((async {
                        // A valid source return/break may make this synthetic
                        // value binding diverge. Only this adapter needs the
                        // generated-code lint exemption.
                        #[allow(clippy::diverging_sub_expression)]
                        #body_binding
                        #check_result
                        #[allow(unreachable_code)]
                        ::core::result::Result::Ok::<_, #error>(#success)
                    }).await?);
                    ::core::result::Result::Ok::<(), #error>(())
                }).await?;
            }
            let #output = #output.expect("completed localside body has a value");
            #value
        });
        prefix.push(Stmt::Expr(
            Expr::Block(syn::ExprBlock {
                attrs: Vec::new(),
                label: None,
                block: lowered,
            }),
            None,
        ));
        prefix
    }
}

impl VisitMut for Lower {
    fn visit_block_mut(&mut self, block: &mut Block) {
        self.block(block, false);
    }

    // An async value or closure has its own return/error boundary. Only an
    // async statement is a declaration; awaited or stored futures are ordinary
    // Rust expressions. Nested functions can opt in with their own attribute.
    fn visit_expr_async_mut(&mut self, _: &mut syn::ExprAsync) {}
    fn visit_expr_closure_mut(&mut self, _: &mut syn::ExprClosure) {}
    fn visit_item_mut(&mut self, _: &mut syn::Item) {}
}

struct Exit {
    key: String,
    variant: Ident,
    ty: Option<Ident>,
    jump: Tokens,
}

struct Exits {
    flow: Ident,
    error: Tokens,
    inner: Vec<(Option<Lifetime>, bool)>,
    exits: Vec<Exit>,
}

impl Exits {
    fn inside(&self, label: &Option<Lifetime>) -> bool {
        self.inner.iter().rev().any(|(inner, is_loop)| match label {
            Some(label) => inner
                .as_ref()
                .is_some_and(|inner| inner.ident == label.ident),
            None => *is_loop,
        })
    }

    fn jump(&mut self, key: String, payload: Option<Expr>, jump: Tokens) -> Expr {
        let index = self
            .exits
            .iter()
            .position(|exit| exit.key == key)
            .unwrap_or_else(|| {
                let index = self.exits.len();
                self.exits.push(Exit {
                    key,
                    variant: format_ident!("Exit{index}"),
                    ty: payload.as_ref().map(|_| format_ident!("T{index}")),
                    jump,
                });
                index
            });
        let variant = &self.exits[index].variant;
        let payload = payload.map(|value| quote!((#value)));
        let flow = &self.flow;
        let error = &self.error;
        parse_quote!(return ::core::result::Result::Ok::<_, #error>(
            ::core::ops::ControlFlow::Break(#flow::#variant #payload)
        ))
    }
}

impl VisitMut for Exits {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        visit_mut::visit_expr_mut(self, expr);
        match expr {
            Expr::Return(ret) => {
                let payload = ret
                    .expr
                    .as_deref()
                    .cloned()
                    .unwrap_or_else(|| parse_quote!(()));
                let error = &self.error;
                let payload = parse_quote!(::core::result::Result::<_, #error>::map(
                    #payload, ::core::result::Result::Ok::<_, #error>
                )?);
                *expr = self.jump("return".into(), Some(payload), quote!(return value));
            }
            Expr::Break(brk) if !self.inside(&brk.label) => {
                let label = &brk.label;
                let payload = brk.expr.as_deref().cloned();
                let key = format!("break:{}:{}", quote!(#label), payload.is_some());
                let jump = if payload.is_some() {
                    quote!(break #label value)
                } else {
                    quote!(break #label)
                };
                *expr = self.jump(key, payload, jump);
            }
            Expr::Continue(cont) if !self.inside(&cont.label) => {
                let label = &cont.label;
                let key = format!("continue:{}", quote!(#label));
                *expr = self.jump(key, None, quote!(continue #label));
            }
            _ => {}
        }
    }

    fn visit_expr_loop_mut(&mut self, expr: &mut syn::ExprLoop) {
        self.inner
            .push((expr.label.as_ref().map(|label| label.name.clone()), true));
        visit_mut::visit_expr_loop_mut(self, expr);
        self.inner.pop();
    }
    fn visit_expr_while_mut(&mut self, expr: &mut syn::ExprWhile) {
        self.inner
            .push((expr.label.as_ref().map(|label| label.name.clone()), true));
        visit_mut::visit_expr_while_mut(self, expr);
        self.inner.pop();
    }
    fn visit_expr_for_loop_mut(&mut self, expr: &mut syn::ExprForLoop) {
        self.inner
            .push((expr.label.as_ref().map(|label| label.name.clone()), true));
        visit_mut::visit_expr_for_loop_mut(self, expr);
        self.inner.pop();
    }
    fn visit_expr_block_mut(&mut self, expr: &mut syn::ExprBlock) {
        self.inner
            .push((expr.label.as_ref().map(|label| label.name.clone()), false));
        visit_mut::visit_expr_block_mut(self, expr);
        self.inner.pop();
    }
    fn visit_expr_async_mut(&mut self, _: &mut syn::ExprAsync) {}
    fn visit_expr_closure_mut(&mut self, _: &mut syn::ExprClosure) {}
    fn visit_item_mut(&mut self, _: &mut syn::Item) {}
}
