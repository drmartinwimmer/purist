use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::punctuated::Punctuated;
use syn::token::Comma;

/// Rule detecting trivial wrapper functions that merely forward arguments to an associated method.
pub struct NoRedundantWrappersRule;

impl Rule for NoRedundantWrappersRule {
    fn name(&self) -> &'static str {
        "opinionated::no_redundant_wrappers"
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

fn is_exempt_fn(item_fn: &syn::ItemFn) -> bool {
    item_fn.attrs.iter().any(|attr| {
        attr.path().is_ident("deprecated")
            || attr.path().is_ident("test")
            || attr
                .path()
                .segments
                .last()
                .map(|s| s.ident == "test")
                .unwrap_or(false)
    })
}

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
        assert_that!(&diag.rule, eq("opinionated::no_redundant_wrappers"));
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
        assert_that!(&diag.rule, eq("opinionated::no_redundant_wrappers"));
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
