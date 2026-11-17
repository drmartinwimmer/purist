//! # Rule: purist::test_matcher_borrow_simplification
//!
//! ## What This Rule Does
//! Flags redundant `.as_str()` conversion calls inside GoogleTest assertions
//! (`assert_that!` and `expect_that!`).
//!
//! ## Why This Rule Exists
//! GoogleTest Rust string matchers (such as `eq(...)`, `contains_substring(...)`, `starts_with(...)`)
//! operate seamlessly on borrowed `&String` references. Calling `.as_str()` adds unnecessary
//! visual clutter and noise to test assertions when the expression can simply be borrowed directly
//! with `&<expr>`.
//!
//! Note: `.as_slice()` is intentionally permitted because slice matchers (such as
//! `contains_subslice(...)` or `eq(&[...])`) require `&[T]`, which `&Vec<T>` does not satisfy
//! without an unidiomatic deref-borrow (`&*vec`).
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! assert_that!(result.as_str(), eq("success"));
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! assert_that!(&result, eq("success"));
//! expect_that!(args.as_slice(), contains_subslice(&["--settings", &local_str]));
//! ```

use crate::checkers::check_macro_matches;
use crate::diagnostics::Span;
use crate::engine::LintContext;
use syn::Token;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging redundant `.as_str()` conversions inside GoogleTest assertions.
pub(crate) struct TestMatcherBorrowSimplificationRule;

impl TestMatcherBorrowSimplificationRule {
    pub(crate) fn check_file(
        &self,
        ctx: &LintContext<'_>,
        file: &syn::File,
    ) -> Vec<TestMatcherBorrowSimplificationDiagnostic> {
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
    diagnostics: Vec<TestMatcherBorrowSimplificationDiagnostic>,
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
    diagnostics: &mut Vec<TestMatcherBorrowSimplificationDiagnostic>,
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

/// Visitor that traverses an expression inside an assertion looking for redundant `.as_str()` calls.
struct BorrowMethodVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<TestMatcherBorrowSimplificationDiagnostic>,
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

/// Emits a diagnostic if the method call is a redundant `.as_str()`.
fn check_method_call_redundant_borrow(
    ctx: &LintContext<'_>,
    call: &syn::ExprMethodCall,
) -> Option<TestMatcherBorrowSimplificationDiagnostic> {
    let method_name = call.method.to_string();
    if method_name == "as_str" {
        let span = ctx.to_span(call.span());
        Some(TestMatcherBorrowSimplificationDiagnostic { method_name, span })
    } else {
        None
    }
}

/// Diagnostic emitted by `purist::test_matcher_borrow_simplification`.
#[purist_derive::rule_diagnostic]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, miette::Diagnostic)]
#[error(
    "Redundant '.{method_name}()' in GoogleTest assertion. GoogleTest matchers accept borrowed references directly."
)]
#[diagnostic(help(
    "Replace '.{method_name}()' with a reference borrowing the expression directly ('&<expr>')."
))]
pub struct TestMatcherBorrowSimplificationDiagnostic {
    pub method_name: String,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use miette::Diagnostic;
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
        let diags = TestMatcherBorrowSimplificationRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            diag.code().map(|c| c.to_string()).unwrap_or_default(),
            eq("purist::test_matcher_borrow_simplification")
        );
        assert_that!(
            diag.to_string(),
            contains_substring("Redundant '.as_str()'")
        );
        Ok(())
    }

    #[googletest::test]
    fn as_slice_in_matcher_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn sample_test() {
    expect_that!(args.as_slice(), contains_subslice(&["--settings", &local_str]));
    expect_that!(actual, eq(expected.as_slice()));
}
"#;
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = TestMatcherBorrowSimplificationRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
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
        let diags = TestMatcherBorrowSimplificationRule.check_file(&ctx, &ast);

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
        let diags = TestMatcherBorrowSimplificationRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
