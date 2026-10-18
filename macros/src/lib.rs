use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote};
use syn::{
    Attribute, Block, Expr, ItemFn, ReturnType, Stmt, Type,
    parse::{ParseStream, Parser},
    parse_macro_input, parse_quote,
    visit_mut::{self, VisitMut},
};

/// Drive localside declarations for the lifetime of their enclosing async body.
#[proc_macro_attribute]
pub fn localside(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(Span::call_site(), "localside takes no arguments")
            .into_compile_error()
            .into();
    }
    expand(parse_macro_input!(input as ItemFn), None)
}

/// A platform entry point, using that image's wake/wait driver.
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
    lower.body(&mut function.block);
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
        .is_some_and(|s| s.ident == "localside")
}

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
            .expect("parsed expression statement round trip"),
    )
}

fn declaration(statement: &Stmt) -> bool {
    matches!(statement, Stmt::Expr(Expr::Async(_), Some(_)))
        || parts(statement).is_some_and(|(attrs, _)| attrs.iter().any(is_marker))
}

impl Lower {
    fn body(&mut self, block: &mut Block) {
        for statement in &mut block.stmts {
            if let Stmt::Expr(Expr::Async(child), Some(_)) = statement {
                // For/while and if-without-else are unit-valued Rust
                // statements even when their optional semicolon is omitted.
                let has_value = match child.block.stmts.last() {
                    Some(Stmt::Expr(Expr::ForLoop(_) | Expr::While(_), None)) => false,
                    Some(Stmt::Expr(Expr::If(branch), None)) => branch.else_branch.is_some(),
                    Some(Stmt::Expr(_, None)) => true,
                    _ => false,
                };
                if !has_value {
                    let error = &self.error;
                    child.block.stmts.push(Stmt::Expr(
                        parse_quote!(
                            ::core::result::Result::Ok::<(), #error>(())
                        ),
                        None,
                    ));
                }
                self.body(&mut child.block);
            } else {
                visit_mut::visit_stmt_mut(self, statement);
            }
        }
        block.stmts = self.statements(core::mem::take(&mut block.stmts));
    }

    fn statements(&mut self, mut statements: Vec<Stmt>) -> Vec<Stmt> {
        let Some(first) = statements.iter().position(declaration) else {
            return statements;
        };
        let mut remaining = statements.split_off(first);
        let count = remaining.iter().take_while(|s| declaration(s)).count();
        let suffix = remaining.split_off(count);
        let name = format_ident!("__hibana_tasks{}", self.next, span = Span::mixed_site());
        let output = format_ident!("__hibana_output{}", self.next, span = Span::mixed_site());
        self.next += 1;
        let bindings: Vec<_> = remaining
            .into_iter()
            .map(|stmt| {
                let (attrs, future) = parts(&stmt).expect("declaration expression");
                let attrs: Vec<_> = attrs
                    .into_iter()
                    .filter(|attr| {
                        if !is_marker(attr) {
                            return true;
                        }
                        if !matches!(attr.meta, syn::Meta::Path(_)) {
                            self.diagnostics.extend(
                                syn::Error::new_spanned(
                                    attr,
                                    "a localside declaration takes no arguments",
                                )
                                .into_compile_error(),
                            );
                        }
                        false
                    })
                    .collect();
                quote!(#(#attrs)* let #name = #name.localside(#future);)
            })
            .collect();
        let error = self.error.clone();
        let setup = quote!(
            let #name = ::hibana_runtime::__private::Runtime::<#error>::new();
            #(#bindings)*
        );
        let tail: Expr = if suffix.is_empty() {
            parse_quote!({ #setup #name.await })
        } else {
            let suffix = Block {
                brace_token: Default::default(),
                stmts: self.statements(suffix),
            };
            // Only an async body is a lifetime/completion boundary. The entire
            // suffix moves into one parent future, so Rust's return/?/loop/label
            // rules need no control-flow rewriting or synthetic exit enums.
            parse_quote!({
                let mut #output = ::core::option::Option::None;
                {
                    #setup
                    #name.localside(async {
                        let value: ::core::result::Result<_, #error> = (async #suffix).await;
                        #output = ::core::option::Option::Some(value?);
                        ::core::result::Result::Ok::<(), #error>(())
                    }).await?;
                }
                ::core::result::Result::Ok::<_, #error>(
                    #output.expect("completed async body produced its value")
                )
            })
        };
        statements.push(Stmt::Expr(tail, None));
        statements
    }
}

impl VisitMut for Lower {
    fn visit_block_mut(&mut self, block: &mut Block) {
        for statement in &mut block.stmts {
            if declaration(statement) {
                self.diagnostics.extend(syn::Error::new_spanned(
                    statement,
                    "localside declarations belong to an async body; ordinary blocks never wait",
                ).into_compile_error());
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
