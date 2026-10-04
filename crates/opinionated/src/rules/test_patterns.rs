//! # Rule: opinionated::test_patterns
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

use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule enforcing test conventions: naming (<verb>_<desc>_<outcome>), googletest assertions, and ? over unwrap.
pub struct TestPatternsRule;

impl Rule for TestPatternsRule {
    fn name(&self) -> &'static str {
        "opinionated::test_patterns"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = TestVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_cfg_test: ctx.is_test_file(),
            current_fn_is_test: false,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects test functions, macros, and unwrap method calls within test scopes.
struct TestVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_cfg_test: bool,
    current_fn_is_test: bool,
}

impl<'ast> Visit<'ast> for TestVisitor<'_> {
    /// Tracks entry into and exit from `#[cfg(test)]` modules.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let is_cfg_test = is_cfg_test_attr(&item_mod.attrs);

        let prev = self.in_cfg_test;
        if is_cfg_test {
            self.in_cfg_test = true;
        }

        visit::visit_item_mod(self, item_mod);
        self.in_cfg_test = prev;
    }

    /// Checks test function naming convention and tracks current test function context.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let has_test_attribute = has_test_attr(&item_fn.attrs);
        let is_test = has_test_attribute
            || (self.in_cfg_test && item_fn.sig.ident.to_string().starts_with("test_"));

        if is_test && let Some(diag) = check_test_name(self.ctx, &item_fn.sig.ident) {
            self.diagnostics.push(diag);
        }

        let prev_fn = self.current_fn_is_test;
        self.current_fn_is_test = is_test;
        visit::visit_item_fn(self, item_fn);
        self.current_fn_is_test = prev_fn;
    }

    /// Checks for standard library assertion macros inside test functions.
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if self.current_fn_is_test
            && let Some(diag) = check_assertion_macro(self.ctx, mac)
        {
            self.diagnostics.push(diag);
        }
        visit::visit_macro(self, mac);
    }

    /// Checks for `.unwrap()` method calls inside test functions.
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if self.current_fn_is_test
            && let Some(diag) = check_unwrap_method_call(self.ctx, call)
        {
            self.diagnostics.push(diag);
        }
        visit::visit_expr_method_call(self, call);
    }
}

/// Checks whether attributes include a `#[cfg(test)]` configuration attribute.
fn is_cfg_test_attr(attrs: &[syn::Attribute]) -> bool {
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

/// Checks whether attributes include a `#[test]` or `#[...::test]` attribute.
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

/// Validates that test function names follow `<verb>_<description>_<outcome>`.
fn check_test_name(ctx: &LintContext<'_>, ident: &syn::Ident) -> Option<Diagnostic> {
    let name = ident.to_string();
    let segments: Vec<&str> = name.split('_').filter(|s| !s.is_empty()).collect();

    if segments.len() < 3 {
        let span = ctx.to_span(ident.span());
        Some(
            Diagnostic::new(
                "opinionated::test_patterns",
                Severity::Warning,
                format!(
                    "Test function '{name}' does not conform to '<verb>_<description>_<outcome>' naming convention."
                ),
            )
            .with_span(span)
            .with_suggested_fix("Rename test to follow '<verb>_<description>_<outcome>' (e.g. 'parse_valid_input_succeeds')."),
        )
    } else {
        None
    }
}

/// Checks if macro invocation is a legacy assertion like `assert_eq!` or `assert_ne!`.
fn check_assertion_macro(ctx: &LintContext<'_>, mac: &syn::Macro) -> Option<Diagnostic> {
    let mac_ident = mac.path.segments.last()?;
    let name = mac_ident.ident.to_string();
    if name == "assert_eq" || name == "assert_ne" {
        let span = ctx.to_span(mac.path.span());
        Some(
            Diagnostic::new(
                "opinionated::test_patterns",
                Severity::Warning,
                format!(
                    "Usage of '{name}!' in test. Use GoogleTest matchers ('assert_that!' or 'expect_that!') instead."
                ),
            )
            .with_span(span)
            .with_suggested_fix("Replace with 'assert_that!(actual, eq(expected))' or 'expect_that!(actual, eq(expected))'."),
        )
    } else {
        None
    }
}

/// Checks if a method call inside a test is `.unwrap()`.
fn check_unwrap_method_call(
    ctx: &LintContext<'_>,
    call: &syn::ExprMethodCall,
) -> Option<Diagnostic> {
    if call.method == "unwrap" {
        let span = ctx.to_span(call.method.span());
        Some(
            Diagnostic::new(
                "opinionated::test_patterns",
                Severity::Warning,
                "Avoid calling '.unwrap()' in test bodies. Propagate errors using '?' or assert with GoogleTest matchers.",
            )
            .with_span(span)
            .with_suggested_fix("Return Result<(), Box<dyn std::error::Error>> or googletest::Result<()> and use '?' instead of '.unwrap()'."),
        )
    } else {
        None
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
        assert_that!(&diag.rule, eq("opinionated::test_patterns"));
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
        assert_that!(&diag.rule, eq("opinionated::test_patterns"));
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
        assert_that!(&diag.rule, eq("opinionated::test_patterns"));
        assert_that!(
            &diag.message,
            contains_substring("Avoid calling '.unwrap()' in test bodies")
        );
        Ok(())
    }
}
