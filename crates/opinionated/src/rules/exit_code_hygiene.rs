use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule enforcing exit code hygiene: no raw integer exits, and no `process::exit` or `ExitCode` in libraries.
pub struct ExitCodeHygieneRule;

impl Rule for ExitCodeHygieneRule {
    fn name(&self) -> &'static str {
        "opinionated::exit_code_hygiene"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let is_main_file = ctx
            .file_path()
            .file_name()
            .map(|f| f == "main.rs")
            .unwrap_or(false);

        let mut visitor = ExitCodeVisitor {
            ctx,
            diagnostics: Vec::new(),
            is_main_file,
            in_main_fn: false,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

struct ExitCodeVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    is_main_file: bool,
    in_main_fn: bool,
}

impl<'ast> Visit<'ast> for ExitCodeVisitor<'_> {
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let is_main = self.is_main_file && item_fn.sig.ident == "main";
        let prev = self.in_main_fn;
        self.in_main_fn = is_main;

        // Check if library function returns ExitCode
        if !self.is_main_file && returns_exit_code(&item_fn.sig.output) {
            let span = self.ctx.to_span(item_fn.sig.output.span());
            let fn_name = item_fn.sig.ident.to_string();
            self.diagnostics.push(
                Diagnostic::new(
                    "opinionated::exit_code_hygiene",
                    Severity::Warning,
                    format!(
                        "Function '{fn_name}' returns 'ExitCode'. Library functions must return 'Result' and let the CLI entrypoint handle exit codes."
                    ),
                )
                .with_span(span)
                .with_suggested_fix("Change return type to 'Result<...>' and propagate errors to the caller."),
            );
        }

        visit::visit_item_fn(self, item_fn);
        self.in_main_fn = prev;
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(expr_path) = &*call.func
            && is_process_exit_path(&expr_path.path)
        {
            let span = self.ctx.to_span(call.span());

            // Check for raw integer literal argument like exit(1) or exit(0)
            if let Some(syn::Expr::Lit(expr_lit)) = call.args.first()
                && matches!(expr_lit.lit, syn::Lit::Int(_))
            {
                self.diagnostics.push(
                    Diagnostic::new(
                        "opinionated::exit_code_hygiene",
                        Severity::Warning,
                        "Raw integer literal passed to 'exit(...)'. Use 'ExitCode::SUCCESS' or 'ExitCode::FAILURE' instead.",
                    )
                    .with_span(span.clone())
                    .with_suggested_fix("Replace integer literal with 'ExitCode::SUCCESS' or 'ExitCode::FAILURE'."),
                );
            }

            // Check if exit is called outside of main function in main.rs
            if !self.in_main_fn {
                self.diagnostics.push(
                    Diagnostic::new(
                        "opinionated::exit_code_hygiene",
                        Severity::Warning,
                        "Direct invocation of 'process::exit' outside 'main()'. Propagate errors using 'Result' instead.",
                    )
                    .with_span(span)
                    .with_suggested_fix("Propagate errors via 'Result' and handle exit codes strictly in 'fn main() -> ExitCode'."),
                );
            }
        }

        visit::visit_expr_call(self, call);
    }
}

fn is_process_exit_path(path: &syn::Path) -> bool {
    let segments: Vec<&syn::PathSegment> = path.segments.iter().collect();
    if let Some(last) = segments.last() {
        if last.ident != "exit" {
            return false;
        }
        if segments.len() == 1 {
            return true;
        }
        if let Some(second) = segments.get(segments.len().saturating_sub(2)) {
            return second.ident == "process";
        }
    }
    false
}

fn returns_exit_code(output: &syn::ReturnType) -> bool {
    if let syn::ReturnType::Type(_, ty) = output
        && let syn::Type::Path(type_path) = &**ty
        && let Some(last) = type_path.path.segments.last()
    {
        return last.ident == "ExitCode";
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn exit_with_integer_in_library_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn abort_execution() {
    std::process::exit(1);
}
"#;
        let ctx = LintContext::new(Path::new("src/engine.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ExitCodeHygieneRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(2)); // Both raw int and non-main call
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::exit_code_hygiene"));
        Ok(())
    }

    #[googletest::test]
    fn library_fn_returning_exit_code_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn execute() -> std::process::ExitCode { ExitCode::SUCCESS }\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ExitCodeHygieneRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::exit_code_hygiene"));
        assert_that!(
            &diag.message,
            contains_substring("Function 'execute' returns 'ExitCode'")
        );
        Ok(())
    }

    #[googletest::test]
    fn main_returning_exit_code_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "fn main() -> std::process::ExitCode { ExitCode::SUCCESS }\n";
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ExitCodeHygieneRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
