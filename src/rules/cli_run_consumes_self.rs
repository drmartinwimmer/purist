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

use super::common::{TestScope, WithTestScope, extract_type_ident};
use crate::checkers::{ReceiverKind, check_fn_receiver};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use crate::scopes::{
    TypeScope, WithTypeScope, is_cli_or_command_struct_name, is_command_execution_fn_name,
};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

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

        let mut visitor = CliRunVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScope::new(ctx.is_test_file()),
            type_scope: TypeScope::new(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

#[derive(WithTestScope, WithTypeScope)]
struct CliRunVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScope,
    type_scope: TypeScope,
}

impl<'ast> Visit<'ast> for CliRunVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });
    }

    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        if let Some(ident) = extract_type_ident(&item_impl.self_ty) {
            self.with_type_impl(ident.to_string(), |this| {
                visit::visit_item_impl(this, item_impl);
            });
        } else {
            visit::visit_item_impl(self, item_impl);
        }
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        self.with_test_fn(&impl_fn.attrs, |this| {
            if !this.test_scope.is_in_test()
                && let Some(struct_name) = this.type_scope.current_impl_name()
                && is_cli_or_command_struct_name(struct_name)
                && let Some(diag) = check_method_receiver_consumes_self(
                    this.ctx,
                    "purist::cli_run_consumes_self",
                    struct_name,
                    impl_fn,
                )
            {
                this.diagnostics.push(diag);
            }

            visit::visit_impl_item_fn(this, impl_fn);
        });
    }
}

/// Checks whether an implementation method on a command struct incorrectly borrows `&self`.
fn check_method_receiver_consumes_self(
    ctx: &LintContext<'_>,
    rule_name: &'static str,
    struct_name: &str,
    impl_fn: &syn::ImplItemFn,
) -> Option<Diagnostic> {
    let fn_name = impl_fn.sig.ident.to_string();
    if !is_command_execution_fn_name(&fn_name) {
        return None;
    }

    let receiver = check_fn_receiver(&impl_fn.sig);
    if !matches!(receiver, ReceiverKind::Ref | ReceiverKind::RefMut) {
        return None;
    }

    let span = ctx.to_span(impl_fn.sig.inputs.first()?.span());
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
