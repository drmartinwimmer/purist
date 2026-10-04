//! Rule: `opinionated::exit_code_hygiene`
//!
//! # What This Rule Does
//! Enforces three key exit code hygiene constraints across application and library code:
//! 1. Bans raw integer literals passed to exit calls (e.g. `std::process::exit(1)`); requires
//!    `ExitCode::SUCCESS` or `ExitCode::FAILURE`.
//! 2. Bans calling `std::process::exit` outside of `fn main()` in `main.rs`.
//! 3. Bans library functions from returning `std::process::ExitCode`.
//!
//! # Why This Rule Exists
//! Direct process exits (`process::exit`) abort execution immediately without unwinding the stack,
//! preventing RAII `Drop` implementations from running (leaving open file locks, orphan subprocesses,
//! and uncleaned temporary directories). In libraries, aborting the process is especially catastrophic
//! because it terminates the entire hosting application without giving callers an opportunity to handle
//! the error. Libraries must return `Result`, leaving process lifecycle control strictly to `main()`.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! // In a library module:
//! pub fn run_checks() {
//!     if failed {
//!         std::process::exit(1); // Aborts host process without cleanup
//!     }
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! // In a library module:
//! pub fn run_checks() -> Result<(), CheckError> {
//!     if failed {
//!         return Err(CheckError::Failed);
//!     }
//!     Ok(())
//! }
//!
//! // In src/main.rs:
//! fn main() -> std::process::ExitCode {
//!     match run_checks() {
//!         Ok(()) => std::process::ExitCode::SUCCESS,
//!         Err(err) => {
//!             eprintln!("Error: {err}");
//!             std::process::ExitCode::FAILURE
//!         }
//!     }
//! }
//! ```

use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
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

/// Visitor inspecting functions and calls for exit code hygiene violations.
struct ExitCodeVisitor<'a> {
    /// Lint context containing file path and coordinate mapping helpers.
    ctx: &'a LintContext<'a>,
    /// Accumulated diagnostic findings.
    diagnostics: Vec<Diagnostic>,
    /// Indicates whether the current file is `main.rs`.
    is_main_file: bool,
    /// Indicates whether the AST traversal is currently inside `fn main()` in `main.rs`.
    in_main_fn: bool,
}

impl<'ast> Visit<'ast> for ExitCodeVisitor<'_> {
    /// Tracks whether traversal is inside `fn main()` and flags library functions returning `ExitCode`.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let is_main = self.is_main_file && item_fn.sig.ident == "main";
        let prev = self.in_main_fn;
        self.in_main_fn = is_main;

        if let Some(diag) = check_library_exit_code_return(self.ctx, self.is_main_file, item_fn) {
            self.diagnostics.push(diag);
        }

        visit::visit_item_fn(self, item_fn);
        self.in_main_fn = prev;
    }

    /// Inspects function call expressions for unhygienic `process::exit` calls or raw integer status codes.
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        let findings = check_process_exit_call(self.ctx, self.in_main_fn, call);
        self.diagnostics.extend(findings);

        visit::visit_expr_call(self, call);
    }
}

/// Checks if a library function inappropriately returns `ExitCode`.
fn check_library_exit_code_return(
    ctx: &LintContext<'_>,
    is_main_file: bool,
    item_fn: &syn::ItemFn,
) -> Option<Diagnostic> {
    if is_main_file || !returns_exit_code(&item_fn.sig.output) {
        return None;
    }

    let span = ctx.to_span(item_fn.sig.output.span());
    let fn_name = item_fn.sig.ident.to_string();
    Some(
        Diagnostic::new(
            "opinionated::exit_code_hygiene",
            Severity::Warning,
            format!(
                "Function '{fn_name}' returns 'ExitCode'. Library functions must return 'Result' and let the CLI entrypoint handle exit codes."
            ),
        )
        .with_span(span)
        .with_suggested_fix("Change return type to 'Result<...>' and propagate errors to the caller."),
    )
}

/// Inspects a call expression for `process::exit`, returning diagnostics for raw integer codes or calls outside `main()`.
fn check_process_exit_call(
    ctx: &LintContext<'_>,
    in_main_fn: bool,
    call: &syn::ExprCall,
) -> Vec<Diagnostic> {
    let mut findings = Vec::new();

    let syn::Expr::Path(expr_path) = &*call.func else {
        return findings;
    };

    if !is_process_exit_path(&expr_path.path) {
        return findings;
    }

    let span = ctx.to_span(call.span());

    // Check for raw integer literal argument like exit(1) or exit(0)
    if let Some(syn::Expr::Lit(expr_lit)) = call.args.first()
        && matches!(expr_lit.lit, syn::Lit::Int(_))
    {
        findings.push(
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
    if !in_main_fn {
        findings.push(
            Diagnostic::new(
                "opinionated::exit_code_hygiene",
                Severity::Warning,
                "Direct invocation of 'process::exit' outside 'main()'. Propagate errors using 'Result' instead.",
            )
            .with_span(span)
            .with_suggested_fix("Propagate errors via 'Result' and handle exit codes strictly in 'fn main() -> ExitCode'."),
        );
    }

    findings
}

/// Checks whether a path resolves to `exit` or `std::process::exit`.
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

/// Returns true if a return type specifies `ExitCode`.
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
