//! # Rule: purist::googletest_conventions
//!
//! ## What This Rule Does
//! Enforces idiomatic GoogleTest suite conventions across Rust test code:
//! 1. Test functions must use the `#[googletest::test]` test harness attribute instead of
//!    standard `#[test]`.
//! 2. Standard library assertion macros (`assert!`, `assert_eq!`, `assert_ne!`) are forbidden
//!    in favor of GoogleTest assertions (`assert_that!` or `expect_that!`).
//! 3. Calling `.expect(...)` in test functions is forbidden in favor of error propagation (`?`)
//!    returning `Result<(), Box<dyn std::error::Error>>` or `googletest::Result<()>`.
//! 4. Binary comparison expressions (e.g. `assert_that!(x == y, is_true())` or `assert_that!(x != y, is_true())`)
//!    and boolean queries (e.g. `assert_that!(x.is_ok(), is_true())`) must use expressive GoogleTest
//!    matchers (`eq(y)`, `not(eq(y))`, `ok(...)`) directly on the operands.
//!
//! ## Why This Rule Exists
//! GoogleTest Rust provides rich failure diagnostics, structured value diffs, and non-fatal failure
//! reporting (`expect_that!`). Using standard `#[test]` bypasses the GoogleTest runner context and
//! failure collector. Using standard `assert!` or `.expect(...)` causes immediate panics with minimal
//! diagnostics and bypasses RAII cleanup guards. Asserting on boolean expressions (like `a == b`) with
//! `is_true()` completely erases the actual values of `a` and `b` on failure, outputting only
//! `Expected: is true, Actual: false`. Using direct matchers preserves full value inspection.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! #[test]
//! fn parse_manifest_fails() {
//!     let config = parse_manifest().expect("manifest parsing failed");
//!     assert!(config.timeout > 0);
//!     assert_that!(config.name == "app", is_true());
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! #[googletest::test]
//! fn parse_manifest_succeeds() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = parse_manifest()?;
//!     assert_that!(config.timeout, gt(0));
//!     assert_that!(config.name, eq("app"));
//!     Ok(())
//! }
//! ```

use super::common::{TestScopeTracker, has_bare_test_attr, has_framework_test_attr, macro_name};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule enforcing GoogleTest suite conventions and discouraging legacy test patterns.
pub struct GoogletestConventionsRule;

impl Rule for GoogletestConventionsRule {
    fn name(&self) -> &'static str {
        "purist::googletest_conventions"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = GoogletestConventionsVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScopeTracker::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects test attributes, assertion macros, and fallible calls within test functions.
struct GoogletestConventionsVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScopeTracker,
}

impl<'ast> Visit<'ast> for GoogletestConventionsVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let _guard = self.test_scope.enter_mod(&item_mod.attrs);
        visit::visit_item_mod(self, item_mod);
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        self.check_bare_test_attr(&item_fn.attrs, &item_fn.sig.ident);
        let fn_name = item_fn.sig.ident.to_string();
        let _guard = self.test_scope.enter_fn_with_name(&item_fn.attrs, &fn_name);
        visit::visit_item_fn(self, item_fn);
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        self.check_bare_test_attr(&impl_fn.attrs, &impl_fn.sig.ident);
        let fn_name = impl_fn.sig.ident.to_string();
        let _guard = self.test_scope.enter_fn_with_name(&impl_fn.attrs, &fn_name);
        visit::visit_impl_item_fn(self, impl_fn);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if !self.test_scope.is_in_test_fn() {
            visit::visit_macro(self, mac);
            return;
        }

        if let Some(mac_ident) = macro_name(mac) {
            let mac_name = mac_ident.to_string();
            if mac_name == "assert" {
                let span = self.ctx.to_span(mac.path.span());
                self.diagnostics.push(
                    Diagnostic::new(
                        "purist::googletest_conventions",
                        Severity::Warning,
                        "Usage of 'assert!' in test. Use GoogleTest matchers ('assert_that!' or 'expect_that!') instead.",
                    )
                    .with_span(span)
                    .with_suggested_fix("Replace with 'assert_that!(actual, matcher)' or 'expect_that!(actual, matcher)'."),
                );
            } else if mac_name == "assert_eq" || mac_name == "assert_ne" {
                let span = self.ctx.to_span(mac.path.span());
                self.diagnostics.push(
                    Diagnostic::new(
                        "purist::googletest_conventions",
                        Severity::Warning,
                        format!(
                            "Usage of '{mac_name}!' in test. Use GoogleTest matchers ('assert_that!' or 'expect_that!') instead."
                        ),
                    )
                    .with_span(span)
                    .with_suggested_fix("Replace with 'assert_that!(actual, eq(expected))'."),
                );
            } else if mac_name == "assert_that" || mac_name == "expect_that" {
                self.check_googletest_assertion_macro(mac);
            }
        }

        visit::visit_macro(self, mac);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if self.test_scope.is_in_test_fn() && call.method == "expect" {
            let span = self.ctx.to_span(call.method.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::googletest_conventions",
                    Severity::Warning,
                    "Avoid calling '.expect(...)' in test bodies. Propagate errors using '?' or assert with GoogleTest matchers.",
                )
                .with_span(span)
                .with_suggested_fix(
                    "Return 'Result<(), Box<dyn std::error::Error>>' or 'googletest::Result<()>' and use '?' instead of '.expect(...)'.",
                ),
            );
        }

        visit::visit_expr_method_call(self, call);
    }
}

impl GoogletestConventionsVisitor<'_> {
    fn check_bare_test_attr(&mut self, attrs: &[syn::Attribute], ident: &syn::Ident) {
        let has_bare_test = has_bare_test_attr(attrs);
        let has_framework_test = has_framework_test_attr(attrs);

        if has_bare_test && !has_framework_test {
            let span = self.ctx.to_span(ident.span());
            let fn_name = ident.to_string();
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::googletest_conventions",
                    Severity::Warning,
                    format!(
                        "Test function '{fn_name}' uses standard '#[test]' attribute. Use '#[googletest::test]' instead."
                    ),
                )
                .with_span(span)
                .with_suggested_fix("Replace '#[test]' with '#[googletest::test]'."),
            );
        }
    }

    fn check_googletest_assertion_macro(&mut self, mac: &syn::Macro) {
        let Ok(args) =
            mac.parse_body_with(Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated)
        else {
            return;
        };

        let args_vec: Vec<_> = args.into_iter().collect();
        let Some(first_expr) = args_vec.first() else {
            return;
        };
        let Some(matcher_expr) = args_vec.get(1) else {
            return;
        };

        if is_true_matcher(matcher_expr) {
            // Check for binary comparison like `a == b` or `a != b`
            if let syn::Expr::Binary(bin) = first_expr {
                match bin.op {
                    syn::BinOp::Eq(_) => {
                        let span = self.ctx.to_span(bin.span());
                        self.diagnostics.push(
                            Diagnostic::new(
                                "purist::googletest_conventions",
                                Severity::Warning,
                                "Assertion compares equality using '=='. Use GoogleTest matcher 'eq(...)' directly on the left operand.",
                            )
                            .with_span(span)
                            .with_suggested_fix("Replace with 'assert_that!(left, eq(right))'."),
                        );
                    }
                    syn::BinOp::Ne(_) => {
                        let span = self.ctx.to_span(bin.span());
                        self.diagnostics.push(
                            Diagnostic::new(
                                "purist::googletest_conventions",
                                Severity::Warning,
                                "Assertion compares inequality using '!='. Use GoogleTest matcher 'not(eq(...))' directly on the left operand.",
                            )
                            .with_span(span)
                            .with_suggested_fix("Replace with 'assert_that!(left, not(eq(right)))'."),
                        );
                    }
                    _ => {}
                }
            } else if let syn::Expr::MethodCall(call) = first_expr {
                let method_name = call.method.to_string();
                let span = self.ctx.to_span(call.span());
                match method_name.as_str() {
                    "is_ok" => {
                        self.diagnostics.push(
                            Diagnostic::new(
                                "purist::googletest_conventions",
                                Severity::Warning,
                                "Assertion checks '.is_ok()' using boolean matcher. Use GoogleTest matcher 'ok(...)' directly on the Result.",
                            )
                            .with_span(span)
                            .with_suggested_fix("Replace with 'assert_that!(target, ok(...))'."),
                        );
                    }
                    "is_err" => {
                        self.diagnostics.push(
                            Diagnostic::new(
                                "purist::googletest_conventions",
                                Severity::Warning,
                                "Assertion checks '.is_err()' using boolean matcher. Use GoogleTest matcher 'err(...)' directly on the Result.",
                            )
                            .with_span(span)
                            .with_suggested_fix("Replace with 'assert_that!(target, err(...))'."),
                        );
                    }
                    "is_some" => {
                        self.diagnostics.push(
                            Diagnostic::new(
                                "purist::googletest_conventions",
                                Severity::Warning,
                                "Assertion checks '.is_some()' using boolean matcher. Use GoogleTest matcher 'some(...)' directly on the Option.",
                            )
                            .with_span(span)
                            .with_suggested_fix("Replace with 'assert_that!(target, some(...))'."),
                        );
                    }
                    "is_none" => {
                        self.diagnostics.push(
                            Diagnostic::new(
                                "purist::googletest_conventions",
                                Severity::Warning,
                                "Assertion checks '.is_none()' using boolean matcher. Use GoogleTest matcher 'none()' directly on the Option.",
                            )
                            .with_span(span)
                            .with_suggested_fix("Replace with 'assert_that!(target, none())'."),
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Checks whether an expression represents `is_true()` or `eq(true)`.
fn is_true_matcher(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(call) => {
            let syn::Expr::Path(path) = &*call.func else {
                return false;
            };
            let Some(ident) = path.path.get_ident() else {
                return false;
            };
            if ident == "is_true" {
                return true;
            }
            if ident == "eq" {
                return match call.args.first() {
                    Some(syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Bool(b),
                        ..
                    })) => b.value,
                    _ => false,
                };
            }
            false
        }
        syn::Expr::Path(path) => path
            .path
            .get_ident()
            .is_some_and(|ident| ident == "is_true"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn bare_test_attribute_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn parse_manifest_succeeds() {}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GoogletestConventionsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::googletest_conventions"));
        assert_that!(
            &diag.message,
            contains_substring("uses standard '#[test]' attribute")
        );
        Ok(())
    }

    #[googletest::test]
    fn googletest_test_attribute_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[googletest::test]
fn parse_manifest_succeeds() {}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GoogletestConventionsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn assert_macro_in_test_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[googletest::test]
fn check_condition_succeeds() {
    assert!(1 > 0);
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GoogletestConventionsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::googletest_conventions"));
        assert_that!(
            &diag.message,
            contains_substring("Usage of 'assert!' in test")
        );
        Ok(())
    }

    #[googletest::test]
    fn assert_eq_macro_in_test_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[googletest::test]
fn check_equality_succeeds() {
    assert_eq!(1, 1);
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GoogletestConventionsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::googletest_conventions"));
        assert_that!(
            &diag.message,
            contains_substring("Usage of 'assert_eq!' in test")
        );
        Ok(())
    }

    #[googletest::test]
    fn expect_call_in_test_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[googletest::test]
fn check_parsing_succeeds() {
    let _val = Some(1).expect("expected value");
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GoogletestConventionsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::googletest_conventions"));
        assert_that!(
            &diag.message,
            contains_substring("Avoid calling '.expect(...)' in test bodies")
        );
        Ok(())
    }

    #[googletest::test]
    fn equality_binary_expr_in_assert_that_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[googletest::test]
fn check_equality_succeeds() {
    assert_that!(1 == 1, is_true());
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GoogletestConventionsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::googletest_conventions"));
        assert_that!(
            &diag.message,
            contains_substring("Assertion compares equality using '=='")
        );
        Ok(())
    }

    #[googletest::test]
    fn inequality_binary_expr_in_assert_that_is_flagged() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = r#"
#[googletest::test]
fn check_inequality_succeeds() {
    assert_that!(1 != 2, is_true());
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GoogletestConventionsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::googletest_conventions"));
        assert_that!(
            &diag.message,
            contains_substring("Assertion compares inequality using '!='")
        );
        Ok(())
    }

    #[googletest::test]
    fn is_ok_boolean_query_in_assert_that_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[googletest::test]
fn check_result_succeeds() {
    let res: Result<i32, String> = Ok(1);
    assert_that!(res.is_ok(), is_true());
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GoogletestConventionsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::googletest_conventions"));
        assert_that!(
            &diag.message,
            contains_substring("Assertion checks '.is_ok()' using boolean matcher")
        );
        Ok(())
    }

    #[googletest::test]
    fn is_some_boolean_query_in_assert_that_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[googletest::test]
fn check_option_succeeds() {
    let opt: Option<i32> = Some(1);
    assert_that!(opt.is_some(), is_true());
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GoogletestConventionsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::googletest_conventions"));
        assert_that!(
            &diag.message,
            contains_substring("Assertion checks '.is_some()' using boolean matcher")
        );
        Ok(())
    }

    #[googletest::test]
    fn idiomatic_googletest_assertions_are_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[googletest::test]
fn check_everything_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    let res: Result<i32, String> = Ok(42);
    let val = res?;
    assert_that!(val, eq(42));
    assert_that!(val > 0, is_true());
    Ok(())
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GoogletestConventionsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
