//! # Rule: purist::no_redundant_wrappers
//!
//! ## What This Rule Does
//! Flags trivial wrapper functions that merely forward arguments directly to another
//! function or method without performing any additional work, transformations, or validation.
//!
//! ## Why This Rule Exists
//! Wrapper functions that do nothing except forward their parameters to an underlying
//! method or function add unnecessary layers of indirection, obscure call graphs,
//! and bloat the API surface. Callers should instead invoke the underlying method directly.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! pub fn forget_workspace(workspace: &Workspace) {
//!     workspace.forget();
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! // Callers invoke the method directly:
//! workspace.forget();
//! ```

use super::common::has_test_attr;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::punctuated::Punctuated;
use syn::token::Comma;

/// Rule detecting trivial wrapper functions that merely forward arguments to an associated method.
pub struct NoRedundantWrappersRule;

impl Rule for NoRedundantWrappersRule {
    fn name(&self) -> &'static str {
        "purist::no_redundant_wrappers"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        if ctx.is_test_file() {
            return diagnostics;
        }

        for item in &file.items {
            if let syn::Item::Fn(item_fn) = item {
                // Ignore deprecated functions or tests
                if is_exempt_fn(item_fn) {
                    continue;
                }

                if let Some(target_desc) = detect_redundant_forwarding(item_fn) {
                    let span = ctx.to_span(item_fn.sig.ident.span());
                    let fn_name = item_fn.sig.ident.to_string();
                    diagnostics.push(
                        Diagnostic::new(
                            self.name(),
                            Severity::Warning,
                            format!(
                                "Function '{fn_name}' is a redundant wrapper that merely forwards to '{target_desc}'. Call '{target_desc}' directly."
                            ),
                        )
                        .with_span(span)
                        .with_suggested_fix(format!(
                            "Remove wrapper function '{fn_name}' and call '{target_desc}' directly at call sites."
                        )),
                    );
                }
            }
        }

        diagnostics
    }
}

/// Returns true if the function is exempt from wrapper checks (e.g. test functions or deprecated APIs).
fn is_exempt_fn(item_fn: &syn::ItemFn) -> bool {
    has_test_attr(&item_fn.attrs)
        || item_fn
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("deprecated"))
}

/// Detects whether a function is a single-statement passthrough forwarding all arguments.
fn detect_redundant_forwarding(item_fn: &syn::ItemFn) -> Option<String> {
    if item_fn.block.stmts.len() != 1 {
        return None;
    }

    let stmt = item_fn.block.stmts.first()?;
    let expr = match stmt {
        syn::Stmt::Expr(e, _) => e,
        _ => return None,
    };

    // Collect parameter names
    let param_names = extract_param_names(&item_fn.sig.inputs);
    if param_names.is_empty() {
        return None;
    }

    match expr {
        syn::Expr::Call(call) => {
            let arg_names = extract_arg_names(&call.args);
            if arg_names == param_names {
                let func_name = expr_to_string(&call.func);
                return Some(func_name);
            }
        }
        syn::Expr::MethodCall(call) => {
            // E.g. workspace.forget() where workspace is the first param
            let receiver_name = expr_to_string(&call.receiver);
            let mut all_args = vec![receiver_name];
            all_args.extend(extract_arg_names(&call.args));

            if all_args == param_names {
                return Some(format!(".{}()", call.method));
            }
        }
        _ => {}
    }

    None
}

/// Extracts parameter identifier names from function inputs.
fn extract_param_names(inputs: &Punctuated<syn::FnArg, Comma>) -> Vec<String> {
    let mut names = Vec::new();
    for input in inputs {
        if let syn::FnArg::Typed(pat_type) = input
            && let syn::Pat::Ident(pat_ident) = &*pat_type.pat
        {
            names.push(pat_ident.ident.to_string());
        }
    }
    names
}

/// Extracts identifier names passed as arguments in a call expression.
fn extract_arg_names(args: &Punctuated<syn::Expr, Comma>) -> Vec<String> {
    let mut names = Vec::new();
    for arg in args {
        match arg {
            syn::Expr::Path(p) => {
                if let Some(ident) = p.path.get_ident() {
                    names.push(ident.to_string());
                }
            }
            syn::Expr::Reference(r) => {
                if let syn::Expr::Path(p) = &*r.expr
                    && let Some(ident) = p.path.get_ident()
                {
                    names.push(ident.to_string());
                }
            }
            _ => {}
        }
    }
    names
}

/// Converts a path expression to its string representation.
fn expr_to_string(expr: &syn::Expr) -> String {
    match expr {
        syn::Expr::Path(p) => p
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
        _ => "target".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn redundant_static_wrapper_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn run(args: Args) -> ExitCode { Cli::run(args) }\n";
        let ctx = LintContext::new(Path::new("src/cli.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoRedundantWrappersRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_redundant_wrappers"));
        assert_that!(&diag.message, contains_substring("redundant wrapper"));
        Ok(())
    }

    #[googletest::test]
    fn redundant_method_wrapper_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn forget_workspace(workspace: &Workspace) { workspace.forget(); }\n";
        let ctx = LintContext::new(Path::new("src/workspace.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoRedundantWrappersRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_redundant_wrappers"));
        assert_that!(&diag.message, contains_substring("redundant wrapper"));
        Ok(())
    }

    #[googletest::test]
    fn wrapper_with_additional_logic_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn run(args: Args) -> ExitCode {
    println!("Starting...");
    Cli::run(args)
}
"#;
        let ctx = LintContext::new(Path::new("src/cli.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoRedundantWrappersRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
