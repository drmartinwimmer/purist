use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;

/// Rule enforcing that CLI command execution methods consume `self` by value rather than `&self`.
pub struct CliRunConsumesSelfRule;

impl Rule for CliRunConsumesSelfRule {
    fn name(&self) -> &'static str {
        "opinionated::cli_run_consumes_self"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        if ctx.is_test_file() {
            return diagnostics;
        }

        for item in &file.items {
            if let syn::Item::Impl(item_impl) = item
                && let syn::Type::Path(type_path) = &*item_impl.self_ty
                && let Some(ident) = type_path.path.get_ident()
            {
                let struct_name = ident.to_string();
                if !is_cli_or_command_struct_name(&struct_name) {
                    continue;
                }

                for impl_item in &item_impl.items {
                    if let syn::ImplItem::Fn(impl_fn) = impl_item {
                        let fn_name = impl_fn.sig.ident.to_string();
                        if is_command_execution_fn_name(&fn_name)
                            && let Some(syn::FnArg::Receiver(recv)) = impl_fn.sig.inputs.first()
                            && recv.reference.is_some()
                        {
                            let span = ctx.to_span(recv.span());
                            diagnostics.push(
                                Diagnostic::new(
                                    self.name(),
                                    Severity::Warning,
                                    format!(
                                        "CLI execution method '{fn_name}' on command struct '{struct_name}' takes '&self' by reference. CLI commands should consume 'self' by value ('pub fn {fn_name}(self, ...)') to take ownership of parsed arguments and avoid unnecessary cloning."
                                    ),
                                )
                                .with_span(span)
                                .with_suggested_fix("Change method receiver from '&self' to 'self'."),
                            );
                        }
                    }
                }
            }
        }

        diagnostics
    }
}

fn is_cli_or_command_struct_name(name: &str) -> bool {
    name.ends_with("Command") || name.ends_with("Cli") || name == "Cli" || name == "Commands"
}

fn is_command_execution_fn_name(name: &str) -> bool {
    name == "run" || name == "run_with_format" || name == "execute"
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn run_taking_borrow_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct BuildCommand;

impl BuildCommand {
    pub fn run(&self) -> Result<(), ()> {
        Ok(())
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/commands/build.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CliRunConsumesSelfRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::cli_run_consumes_self"));
        assert_that!(
            &diag.message,
            contains_substring("takes '&self' by reference")
        );
        Ok(())
    }

    #[googletest::test]
    fn run_taking_self_by_value_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct BuildCommand;

impl BuildCommand {
    pub fn run(self) -> Result<(), ()> {
        Ok(())
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/commands/build.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CliRunConsumesSelfRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn non_command_struct_run_taking_borrow_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct ProcessRunner;

impl ProcessRunner {
    pub fn run(&self) {}
}
"#;
        let ctx = LintContext::new(Path::new("src/runner.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CliRunConsumesSelfRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
