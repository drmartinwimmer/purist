//! # Rule: purist::test_patterns
//!
//! ## What This Rule Does
//! Enforces three key testing conventions across test suites:
//! 1. Standardized naming: Test function names must follow `<verb>_<description>_<outcome>`
//!    (e.g., `parse_valid_manifest_succeeds`).
//! 2. GoogleTest assertions: Forbids `assert_eq!` and `assert_ne!`, requiring GoogleTest
//!    matchers (`assert_that!` or `expect_that!`).
//! 3. Clean error propagation: Forbids calling `.unwrap()` in test bodies, recommending returning
//!    `Result<(), ...>` with `?` or using GoogleTest matchers.
//!
//! ## Why This Rule Exists
//! Consistent test naming ensures test suite outputs and CI failure summaries are readable
//! at a glance. GoogleTest matchers produce rich diagnostic output, structured diffs, and support
//! clean matcher composition. Replacing `.unwrap()` panics with error propagation (`?`) produces
//! clean backtraces and preserves RAII guard cleanup without sudden panics.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! #[test]
//! fn test_parse() {
//!     let config = parse_config().unwrap();
//!     assert_eq!(config.timeout, 30);
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! #[test]
//! fn parse_valid_config_succeeds() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = parse_config()?;
//!     assert_that!(config.timeout, eq(30));
//!     Ok(())
//! }
//! ```

use super::common::{TestScope, WithTestScope};
use crate::checkers::{check_macro_matches, check_method_call_matches_name};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule enforcing test conventions: naming (<verb>_<desc>_<outcome>), googletest assertions, and ? over unwrap.
pub struct TestPatternsRule;

impl Rule for TestPatternsRule {
    fn name(&self) -> &'static str {
        "purist::test_patterns"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = TestVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScope::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects test functions, macros, and unwrap method calls within test scopes.
#[derive(WithTestScope)]
struct TestVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScope,
}

impl<'ast> Visit<'ast> for TestVisitor<'_> {
    /// Tracks entry into and exit from `#[cfg(test)]` modules.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });
    }

    /// Checks test function naming convention and tracks current test function context.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let fn_name = item_fn.sig.ident.to_string();
        self.with_test_fn_with_name(&item_fn.attrs, &fn_name, |this| {
            if this.test_scope.is_in_test_fn() {
                this.check_test_fn_naming_convention(&item_fn.sig.ident);
            }

            visit::visit_item_fn(this, item_fn);
        });
    }

    /// Checks for standard library assertion macros inside test functions.
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if self.test_scope.is_in_test_fn() {
            self.check_macro_assertion_in_test(mac);
        }
        visit::visit_macro(self, mac);
    }

    /// Checks for `.unwrap()` method calls inside test functions.
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if self.test_scope.is_in_test_fn() {
            self.check_method_call_unwrap_in_test(call);
        }
        visit::visit_expr_method_call(self, call);
    }
}

impl TestVisitor<'_> {
    /// Validates that test function names follow `<verb>_<description>_<outcome>`.
    fn check_test_fn_naming_convention(&mut self, ident: &syn::Ident) {
        let name = ident.to_string();
        let segments: Vec<&str> = name.split('_').filter(|s| !s.is_empty()).collect();

        if segments.len() < 3 {
            let span = self.ctx.to_span(ident.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::test_patterns",
                    Severity::Warning,
                    format!(
                        "Test function '{name}' does not conform to '<verb>_<description>_<outcome>' naming convention."
                    ),
                )
                .with_span(span)
                .with_suggested_fix("Rename test to follow '<verb>_<description>_<outcome>' (e.g. 'parse_valid_input_succeeds')."),
            );
        }
    }

    /// Checks if macro invocation is a legacy assertion like `assert_eq!` or `assert_ne!`.
    fn check_macro_assertion_in_test(&mut self, mac: &syn::Macro) {
        const TARGETS: &[&str] = &["assert_eq", "assert_ne"];
        let Some(name) = check_macro_matches(mac, TARGETS) else {
            return;
        };

        let span = self.ctx.to_span(mac.path.span());
        self.diagnostics.push(
            Diagnostic::new(
                "purist::test_patterns",
                Severity::Warning,
                format!(
                    "Usage of '{name}!' in test. Use GoogleTest matchers ('assert_that!' or 'expect_that!') instead."
                ),
            )
            .with_span(span)
            .with_suggested_fix("Replace with 'assert_that!(actual, eq(expected))' or 'expect_that!(actual, eq(expected))'."),
        );
    }

    /// Checks if a method call inside a test is `.unwrap()`.
    fn check_method_call_unwrap_in_test(&mut self, call: &syn::ExprMethodCall) {
        if check_method_call_matches_name(call, &["unwrap"]) {
            let span = self.ctx.to_span(call.method.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::test_patterns",
                    Severity::Warning,
                    "Avoid calling '.unwrap()' in test bodies. Propagate errors using '?' or assert with GoogleTest matchers.",
                )
                .with_span(span)
                .with_suggested_fix("Return Result<(), Box<dyn std::error::Error>> or googletest::Result<()> and use '?' instead of '.unwrap()'."),
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
    fn short_test_name_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "#[test]\nfn test_parse() {}\n";
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = TestPatternsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::test_patterns"));
        assert_that!(
            &diag.message,
            contains_substring("does not conform to '<verb>_<description>_<outcome>'")
        );
        Ok(())
    }

    #[googletest::test]
    fn valid_three_part_test_name_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "#[test]\nfn parse_valid_manifest_succeeds() {}\n";
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = TestPatternsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn assert_eq_in_test_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "#[test]\nfn verify_result_matches_expected() {\n    assert_eq!(1, 1);\n}\n";
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = TestPatternsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::test_patterns"));
        assert_that!(
            &diag.message,
            contains_substring("Usage of 'assert_eq!' in test")
        );
        Ok(())
    }

    #[googletest::test]
    fn unwrap_in_test_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source =
            "#[test]\nfn check_parse_returns_value() {\n    let val = Some(42).unwrap();\n}\n";
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = TestPatternsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::test_patterns"));
        assert_that!(
            &diag.message,
            contains_substring("Avoid calling '.unwrap()' in test bodies")
        );
        Ok(())
    }
}
