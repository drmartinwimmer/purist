//! Rule: `purist::no_println_in_libraries`
//!
//! # What This Rule Does
//! Flags direct invocations of `println!` and `eprintln!` in library modules. Dedicated entry points
//! (`main.rs`, `/bin/`, `/examples/`), test scopes, and CLI command execution runners (`run`,
//! `run_with_format`, `execute` on command structs) are exempt.
//!
//! # Why This Rule Exists
//! Libraries should never write directly to standard output or standard error. Doing so:
//! 1. Corrupts structured machine-readable stdout (e.g. JSON or CSV pipelines).
//! 2. Prevents consuming applications from configuring output format, log levels, or redirection.
//! 3. Can cause unexpected panics in multithreaded environments when stdout is closed.
//!
//! Libraries must communicate through structured return types (`Result`), diagnostics collectors, or
//! the `tracing`/`log` facades.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! // In a core parsing library:
//! pub fn parse_document(input: &str) -> Document {
//!     println!("Parsing {} bytes...", input.len()); // Pollutes stdout
//!     ...
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! use tracing::info;
//!
//! pub fn parse_document(input: &str) -> Document {
//!     info!(bytes = input.len(), "parsing document"); // Clean structured logging
//!     ...
//! }
//! ```

use super::common::{TestScopeTracker, extract_type_ident, macro_name, path_last_ident};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging `println!` / `eprintln!` in library modules.
pub struct NoPrintlnInLibrariesRule;

impl Rule for NoPrintlnInLibrariesRule {
    fn name(&self) -> &'static str {
        "purist::no_println_in_libraries"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if is_exempt_entrypoint_or_example(ctx) {
            return Vec::new();
        }

        let mut visitor = PrintlnVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScopeTracker::new(ctx.is_test_file()),
            current_impl_is_cli: false,
            in_cli_runner_scope: false,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Returns true if the file is an entrypoint binary or example where standard output printing is expected.
fn is_exempt_entrypoint_or_example(ctx: &LintContext<'_>) -> bool {
    let path_str = ctx.file_path().to_string_lossy();
    path_str.ends_with("main.rs") || path_str.contains("/bin/") || path_str.contains("/examples/")
}

/// Returns true if the type name matches CLI command or argument conventions.
fn is_cli_type_name(name: &str) -> bool {
    name.ends_with("Command")
        || name.ends_with("Cli")
        || name == "Cli"
        || name == "Commands"
        || name.ends_with("Subcommand")
}

/// Returns true if an impl block represents a CLI command or command runner.
fn is_cli_impl(item_impl: &syn::ItemImpl) -> bool {
    if let Some(ident) = extract_type_ident(&item_impl.self_ty)
        && is_cli_type_name(&ident.to_string())
    {
        return true;
    }
    if let Some((ref trait_path, _)) = item_impl.trait_
        && let Some(ident) = path_last_ident(trait_path)
    {
        let name = ident.to_string();
        if name.ends_with("Command") || name == "Command" || name.ends_with("Runner") {
            return true;
        }
    }
    false
}

/// Returns true if the identifier matches CLI command runner method names.
fn is_execution_method(ident: &syn::Ident) -> bool {
    let name = ident.to_string();
    matches!(name.as_str(), "run" | "run_with_format" | "execute")
}

/// Visitor that inspects macro invocations for `println!` and `eprintln!` while tracking CLI runner and test scopes.
struct PrintlnVisitor<'a> {
    /// Lint context containing file path and coordinate mapping helpers.
    ctx: &'a LintContext<'a>,
    /// Accumulated diagnostic findings.
    diagnostics: Vec<Diagnostic>,
    /// Tracks active test scope across modules and test functions.
    test_scope: TestScopeTracker,
    /// Indicates whether traversal is currently within an impl block for a CLI struct.
    current_impl_is_cli: bool,
    /// Indicates whether traversal is currently within an execution runner method (`run`, `execute`).
    in_cli_runner_scope: bool,
}

impl<'ast> Visit<'ast> for PrintlnVisitor<'_> {
    /// Tracks module scope and marks test scope active if annotated with `#[cfg(test)]`.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let prev = self.test_scope.enter_mod(&item_mod.attrs);
        visit::visit_item_mod(self, item_mod);
        self.test_scope.exit_mod(prev);
    }

    /// Tracks whether traversal is inside an impl block for a CLI command struct.
    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        let prev_impl_is_cli = self.current_impl_is_cli;
        if is_cli_impl(item_impl) {
            self.current_impl_is_cli = true;
        }

        visit::visit_item_impl(self, item_impl);
        self.current_impl_is_cli = prev_impl_is_cli;
    }

    /// Tracks entry into methods, noting whether the method is a CLI runner (`run`, `execute`) or a test.
    fn visit_impl_item_fn(&mut self, method: &'ast syn::ImplItemFn) {
        let prev_test = self.test_scope.enter_fn(&method.attrs);
        let is_runner = self.current_impl_is_cli && is_execution_method(&method.sig.ident);
        let prev_runner = self.in_cli_runner_scope;

        if is_runner {
            self.in_cli_runner_scope = true;
        }

        visit::visit_impl_item_fn(self, method);

        self.test_scope.exit_fn(prev_test);
        self.in_cli_runner_scope = prev_runner;
    }

    /// Tracks entry into free functions, updating test scope if annotated with `#[test]`.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let prev = self.test_scope.enter_fn(&item_fn.attrs);
        visit::visit_item_fn(self, item_fn);
        self.test_scope.exit_fn(prev);
    }

    /// Inspects macro calls and flags `println!` or `eprintln!` in library code outside allowed scopes.
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if let Some(diag) = check_print_macro(
            self.ctx,
            self.test_scope.is_in_test(),
            self.in_cli_runner_scope,
            mac,
        ) {
            self.diagnostics.push(diag);
        }

        visit::visit_macro(self, mac);
    }
}

/// Checks whether a macro invocation is `println!` or `eprintln!` in an unexempt library scope.
fn check_print_macro(
    ctx: &LintContext<'_>,
    in_test_scope: bool,
    in_cli_runner_scope: bool,
    mac: &syn::Macro,
) -> Option<Diagnostic> {
    if in_test_scope || in_cli_runner_scope {
        return None;
    }

    let name = macro_name(mac)?.to_string();

    if name != "println" && name != "eprintln" {
        return None;
    }

    let span = ctx.to_span(mac.path.span());
    Some(
        Diagnostic::new(
            "purist::no_println_in_libraries",
            Severity::Warning,
            format!(
                "Direct use of '{name}!' in library code. Library code should return structured errors or use logging."
            ),
        )
        .with_span(span)
        .with_suggested_fix("Remove print statement; propagate information via 'Result' or use 'tracing'/'log'."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn println_in_library_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn parse_input() {
    println!("Parsing started...");
}
"#;
        let ctx = LintContext::new(Path::new("src/parser.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrintlnInLibrariesRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_println_in_libraries"));
        assert_that!(&diag.message, contains_substring("println!"));
        Ok(())
    }

    #[googletest::test]
    fn println_in_main_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn main() {
    println!("Hello, CLI!");
}
"#;
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrintlnInLibrariesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn println_in_test_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn runs_test() {
    println!("debug test info");
}
"#;
        let ctx = LintContext::new(Path::new("src/parser.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrintlnInLibrariesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn println_in_command_runner_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct ApiCommand {
    pub quiet: bool,
}

impl ApiCommand {
    pub fn run(self) -> Result<(), ApiError> {
        if !self.quiet {
            eprintln!("Notice: api drift detector is scheduled for future milestones.");
        }
        Ok(())
    }
}
"#;
        let ctx = LintContext::new(Path::new("crates/api/src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrintlnInLibrariesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn println_in_cli_runner_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Cli;

impl Cli {
    pub fn run(self) -> Result<(), String> {
        eprintln!("Error executing command");
        Ok(())
    }
}
"#;
        let ctx = LintContext::new(Path::new("crates/code-review/src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrintlnInLibrariesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
