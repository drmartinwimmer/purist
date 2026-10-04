//! Rule: `opinionated::centralized_command_execution`
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

use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging raw `Command::new` invocations outside dedicated command/tool modules.
pub struct CentralizedCommandsRule;

impl Rule for CentralizedCommandsRule {
    fn name(&self) -> &'static str {
        "opinionated::centralized_command_execution"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if is_exempt_path(ctx) {
            return Vec::new();
        }

        let mut visitor = CommandVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_test_scope: false,
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
    /// Indicates whether the AST traversal is currently inside a test function or test module.
    in_test_scope: bool,
}

impl<'ast> Visit<'ast> for CommandVisitor<'_> {
    /// Tracks entry into and exit from modules, updating test scope if annotated with `#[cfg(test)]`.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let is_cfg_test = has_cfg_test_attr(&item_mod.attrs);
        let prev = self.in_test_scope;
        if is_cfg_test {
            self.in_test_scope = true;
        }

        visit::visit_item_mod(self, item_mod);
        self.in_test_scope = prev;
    }

    /// Tracks entry into and exit from functions, updating test scope if annotated with `#[test]` or `#[googletest::test]`.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let is_test = has_test_attr(&item_fn.attrs);
        let prev = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }

        visit::visit_item_fn(self, item_fn);
        self.in_test_scope = prev;
    }

    /// Inspects function call expressions and records a diagnostic if an uncentralized `Command::new` is detected.
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if !self.in_test_scope && is_uncentralized_command_call(call) {
            self.diagnostics.push(build_diagnostic(self.ctx, call));
        }

        visit::visit_expr_call(self, call);
    }
}

/// Checks if any attribute matches `#[cfg(test)]`.
fn has_cfg_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        let mut test_attr = false;
        let _result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("test") {
                test_attr = true;
            }
            Ok(())
        });
        test_attr
    })
}

/// Checks if any attribute matches `#[test]` or an attribute ending in `::test`.
fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("test")
            || attr
                .path()
                .segments
                .last()
                .map(|s| s.ident == "test")
                .unwrap_or(false)
    })
}

/// Returns true if a call expression targets `Command::new`.
fn is_uncentralized_command_call(call: &syn::ExprCall) -> bool {
    let syn::Expr::Path(expr_path) = &*call.func else {
        return false;
    };
    is_command_new_path(&expr_path.path)
}

/// Checks if the path resolves to `Command::new` or `std::process::Command::new`.
fn is_command_new_path(path: &syn::Path) -> bool {
    let segments: Vec<&syn::PathSegment> = path.segments.iter().collect();
    if segments.len() < 2 {
        return false;
    }

    let last = segments.last();
    let second_to_last = segments.get(segments.len().saturating_sub(2));

    if let (Some(l), Some(s)) = (last, second_to_last) {
        l.ident == "new" && s.ident == "Command"
    } else {
        false
    }
}

/// Builds the diagnostic finding for an uncentralized `Command::new` invocation.
fn build_diagnostic(ctx: &LintContext<'_>, call: &syn::ExprCall) -> Diagnostic {
    let span = ctx.to_span(call.span());
    Diagnostic::new(
        "opinionated::centralized_command_execution",
        Severity::Warning,
        "Direct invocation of 'Command::new' outside dedicated command/tool module. Encapsulate external process execution in a dedicated tool struct.",
    )
    .with_span(span)
    .with_suggested_fix("Encapsulate command execution and stdout parsing in a dedicated tool builder struct in a 'tools' or 'commands' module.")
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
        assert_that!(&diag.rule, eq("opinionated::centralized_command_execution"));
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
