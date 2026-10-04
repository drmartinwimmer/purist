//! Rule: `purist::cli_run_consumes_self`
//!
//! # What This Rule Does
//! Enforces that CLI execution methods (such as `run`, `run_with_format`, or `execute`) defined on
//! command structs (structs whose names end in `Command` or `Cli`, or named `Cli` or `Commands`)
//! consume `self` by value rather than borrowing `&self`.
//!
//! # Why This Rule Exists
//! CLI execution methods represent the terminal action of a command invocation. Taking `self` by value:
//! 1. Allows moving owned configuration values, file paths, and argument buffers directly into
//!    downstream engines without redundant `.clone()` operations.
//! 2. Prevents re-executing or mutating an already-consumed command instance.
//! 3. Standardizes command method signatures across all CLI toolkits in the workspace.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! pub struct BuildCommand {
//!     target: PathBuf,
//! }
//!
//! impl BuildCommand {
//!     // Borrows &self, requiring cloning `self.target` inside
//!     pub fn run(&self) -> Result<(), BuildError> {
//!         compile(self.target.clone())
//!     }
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! pub struct BuildCommand {
//!     target: PathBuf,
//! }
//!
//! impl BuildCommand {
//!     // Consumes self by value, allowing zero-copy moves of owned fields
//!     pub fn run(self) -> Result<(), BuildError> {
//!         compile(self.target)
//!     }
//! }
//! ```

use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;

/// Rule enforcing that CLI command execution methods consume `self` by value rather than `&self`.
pub struct CliRunConsumesSelfRule;

impl Rule for CliRunConsumesSelfRule {
    fn name(&self) -> &'static str {
        "purist::cli_run_consumes_self"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut diagnostics = Vec::new();

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
                    if let syn::ImplItem::Fn(impl_fn) = impl_item
                        && let Some(diag) =
                            check_method_receiver(ctx, self.name(), &struct_name, impl_fn)
                    {
                        diagnostics.push(diag);
                    }
                }
            }
        }

        diagnostics
    }
}

/// Checks whether an implementation method on a command struct incorrectly borrows `&self`.
fn check_method_receiver(
    ctx: &LintContext<'_>,
    rule_name: &'static str,
    struct_name: &str,
    impl_fn: &syn::ImplItemFn,
) -> Option<Diagnostic> {
    let fn_name = impl_fn.sig.ident.to_string();
    if !is_command_execution_fn_name(&fn_name) {
        return None;
    }

    let first_arg = impl_fn.sig.inputs.first()?;
    let syn::FnArg::Receiver(recv) = first_arg else {
        return None;
    };

    recv.reference.as_ref()?;

    let span = ctx.to_span(recv.span());
    Some(
        Diagnostic::new(
            rule_name,
            Severity::Warning,
            format!(
                "CLI execution method '{fn_name}' on command struct '{struct_name}' takes '&self' by reference. CLI commands should consume 'self' by value ('pub fn {fn_name}(self, ...)') to take ownership of parsed arguments and avoid unnecessary cloning."
            ),
        )
        .with_span(span)
        .with_suggested_fix("Change method receiver from '&self' to 'self'."),
    )
}

/// Returns true if the type name matches CLI command conventions.
fn is_cli_or_command_struct_name(name: &str) -> bool {
    name.ends_with("Command") || name.ends_with("Cli") || name == "Cli" || name == "Commands"
}

/// Returns true if the function name represents a command execution runner.
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
        assert_that!(&diag.rule, eq("purist::cli_run_consumes_self"));
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
