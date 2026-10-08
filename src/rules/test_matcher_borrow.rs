//! # Rule: purist::test_matcher_borrow_simplification
//!
//! ## What This Rule Does
//! Flags redundant `.as_str()` or `.as_slice()` conversion calls inside GoogleTest assertions
//! (`assert_that!` and `expect_that!`).
//!
//! ## Why This Rule Exists
//! GoogleTest Rust matchers (such as `eq(...)`, `contains_substring(...)`, `elements_are!...`)
//! operate seamlessly on borrowed references. Calling `.as_str()` or `.as_slice()` adds unnecessary
//! visual clutter and noise to test assertions. The expression can simply be borrowed directly
//! with `&<expr>`.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! assert_that!(result.as_str(), eq("success"));
//! expect_that!(items.as_slice(), elements_are![eq(&1), eq(&2)]);
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! assert_that!(&result, eq("success"));
//! expect_that!(&items, elements_are![eq(&1), eq(&2)]);
//! ```

use crate::checkers::check_macro_matches;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::Token;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging redundant `.as_str()` or `.as_slice()` conversions inside GoogleTest assertions.
pub struct TestMatcherBorrowRule;

impl Rule for TestMatcherBorrowRule {
    fn name(&self) -> &'static str {
        "purist::test_matcher_borrow_simplification"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = MatcherBorrowVisitor {
            ctx,
            diagnostics: Vec::new(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that identifies GoogleTest assertion macros and searches for redundant borrow method calls.
struct MatcherBorrowVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for MatcherBorrowVisitor<'_> {
    /// Inspects macro invocations for GoogleTest `assert_that!` and `expect_that!`.
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        const TARGETS: &[&str] = &["assert_that", "expect_that"];
        if check_macro_matches(mac, TARGETS).is_some() {
            collect_macro_diagnostics(self.ctx, mac, &mut self.diagnostics);
        }

        visit::visit_macro(self, mac);
    }
}

/// Parses the token stream of an assertion macro and traverses its expressions.
fn collect_macro_diagnostics(
    ctx: &LintContext<'_>,
    mac: &syn::Macro,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let parser = Punctuated::<syn::Expr, Token![,]>::parse_terminated;
    if let Ok(exprs) = Parser::parse2(parser, mac.tokens.clone()) {
        for expr in &exprs {
            let mut method_visitor = BorrowMethodVisitor {
                ctx,
                diagnostics: Vec::new(),
            };
            method_visitor.visit_expr(expr);
            diagnostics.extend(method_visitor.diagnostics);
        }
    }
}

/// Visitor that traverses an expression inside an assertion looking for `.as_str()` or `.as_slice()`.
struct BorrowMethodVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for BorrowMethodVisitor<'_> {
    /// Skips traversing into nested closures (e.g. within iterator combinators).
    fn visit_expr_closure(&mut self, _closure: &'ast syn::ExprClosure) {
        // Do not traverse into closures like .map(|x| x.as_str())
    }

    /// Inspects method calls and flags redundant borrow conversions.
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if let Some(diag) = check_method_call_redundant_borrow(self.ctx, call) {
            self.diagnostics.push(diag);
        }

        visit::visit_expr_method_call(self, call);
    }
}

/// Emits a diagnostic if the method call is a redundant `.as_str()` or `.as_slice()`.
fn check_method_call_redundant_borrow(
    ctx: &LintContext<'_>,
    call: &syn::ExprMethodCall,
) -> Option<Diagnostic> {
    let method_name = call.method.to_string();
    if method_name == "as_str" || method_name == "as_slice" {
        let span = ctx.to_span(call.span());
        Some(
            Diagnostic::new(
                "purist::test_matcher_borrow_simplification",
                Severity::Warning,
                format!(
                    "Redundant '.{method_name}()' in GoogleTest assertion. GoogleTest matchers accept borrowed references directly."
                ),
            )
            .with_span(span)
            .with_suggested_fix(format!(
                "Replace '.{method_name}()' with a reference borrowing the expression directly ('&<expr>')."
            )),
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
    fn as_str_in_assert_that_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn sample_test() {
    assert_that!(result.as_str(), eq("success"));
}
"#;
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = TestMatcherBorrowRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::test_matcher_borrow_simplification"));
        assert_that!(&diag.message, contains_substring("Redundant '.as_str()'"));
        Ok(())
    }

    #[googletest::test]
    fn as_slice_in_matcher_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn sample_test() {
    expect_that!(actual, eq(expected.as_slice()));
}
"#;
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = TestMatcherBorrowRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::test_matcher_borrow_simplification"));
        assert_that!(&diag.message, contains_substring("Redundant '.as_slice()'"));
        Ok(())
    }

    #[googletest::test]
    fn as_ref_in_matcher_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn sample_test() {
    expect_that!(actual.as_ref(), eq(Some(&expected)));
}
"#;
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = TestMatcherBorrowRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn direct_borrow_in_assert_that_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn sample_test() {
    assert_that!(&result, eq("success"));
}
"#;
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = TestMatcherBorrowRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
