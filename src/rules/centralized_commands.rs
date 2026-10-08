//! Rule: `purist::centralized_command_execution`
//!
//! # What This Rule Does
//! Flags direct invocations of `std::process::Command::new` outside of dedicated command execution
//! modules (such as directories or files named `tools/`, `commands/`, `cmd/`, `*_tool.rs`, or `*_command.rs`).
//!
//! # Why This Rule Exists
//! Spreading raw process invocations across business logic makes mocking, error handling, exit code
//! management, environment isolation, and argument sanitization inconsistent and fragile. External
//! commands should be centralized into dedicated tool adapter structs or command runner modules that
//! handle argument composition, timeout limits, stdout/stderr streaming, and structured result parsing.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! pub fn sync_repository() {
//!     // Direct process invocation inside a domain service
//!     let _ = std::process::Command::new("git").arg("fetch").status();
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! // In src/tools/git.rs or src/commands/git.rs
//! pub struct GitTool;
//!
//! impl GitTool {
//!     pub fn fetch(&self) -> io::Result<()> {
//!         let status = std::process::Command::new("git").arg("fetch").status()?;
//!         // Structured error handling...
//!         Ok(())
//!     }
//! }
//! ```

use super::common::TestScopeTracker;
use crate::checkers::check_call_matches_path;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging raw `Command::new` invocations outside dedicated command/tool modules.
pub struct CentralizedCommandsRule;

impl Rule for CentralizedCommandsRule {
    fn name(&self) -> &'static str {
        "purist::centralized_command_execution"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if is_exempt_path(ctx) {
            return Vec::new();
        }

        let mut visitor = CommandVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScopeTracker::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Returns true if the file path is exempt from the centralized command rule (e.g. tests or dedicated tool modules).
fn is_exempt_path(ctx: &LintContext<'_>) -> bool {
    if ctx.is_test_file() {
        return true;
    }

    let path_str = ctx.file_path().to_string_lossy();
    path_str.contains("/tools/")
        || path_str.contains("/commands/")
        || path_str.contains("/cmd/")
        || path_str.ends_with("_tool.rs")
        || path_str.ends_with("_command.rs")
        || path_str.ends_with("/tool.rs")
        || path_str.ends_with("/command.rs")
}

/// Visitor that inspects function calls for direct `Command::new` invocations while tracking test scopes.
struct CommandVisitor<'a> {
    /// Context containing file metadata and source span converters.
    ctx: &'a LintContext<'a>,
    /// Accumulated diagnostic findings.
    diagnostics: Vec<Diagnostic>,
    /// Tracks active test scope across modules and test functions.
    test_scope: TestScopeTracker,
}

impl<'ast> Visit<'ast> for CommandVisitor<'_> {
    /// Tracks entry into and exit from modules, updating test scope if annotated with `#[cfg(test)]`.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.test_scope.push_mod(&item_mod.attrs);
        visit::visit_item_mod(self, item_mod);
        self.test_scope.pop();
    }

    /// Tracks entry into and exit from functions, updating test scope if annotated with `#[test]` or `#[googletest::test]`.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        self.test_scope.push_fn(&item_fn.attrs);
        visit::visit_item_fn(self, item_fn);
        self.test_scope.pop();
    }

    /// Inspects function call expressions and records a diagnostic if an uncentralized `Command::new` is detected.
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if !self.test_scope.is_in_test() {
            self.check_call_uncentralized_command(call);
        }

        visit::visit_expr_call(self, call);
    }
}

impl CommandVisitor<'_> {
    fn check_call_uncentralized_command(&mut self, call: &syn::ExprCall) {
        const TARGETS: &[&[&str]] = &[&["Command", "new"]];
        if check_call_matches_path(call, TARGETS) {
            let span = self.ctx.to_span(call.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::centralized_command_execution",
                    Severity::Warning,
                    "Direct invocation of 'Command::new' outside dedicated command/tool module. Encapsulate external process execution in a dedicated tool struct.",
                )
                .with_span(span)
                .with_suggested_fix("Encapsulate command execution and stdout parsing in a dedicated tool builder struct in a 'tools' or 'commands' module."),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn command_new_in_service_module_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn check_git() {
    let _ = std::process::Command::new("git").status();
}
"#;
        let ctx = LintContext::new(Path::new("src/services/git.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CentralizedCommandsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::centralized_command_execution"));
        Ok(())
    }

    #[googletest::test]
    fn command_new_in_tools_module_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn run_cargo() {
    let _ = std::process::Command::new("cargo").status();
}
"#;
        let ctx = LintContext::new(Path::new("src/tools/cargo.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CentralizedCommandsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn command_new_in_test_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn execute_git_works() {
    let _ = Command::new("git").status();
}
"#;
        let ctx = LintContext::new(Path::new("src/services/git.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CentralizedCommandsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
