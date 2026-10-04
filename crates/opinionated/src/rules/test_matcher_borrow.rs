use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::Token;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging redundant `.as_str()`, `.as_slice()`, or `.as_ref()` conversions inside GoogleTest assertions.
pub struct TestMatcherBorrowRule;

impl Rule for TestMatcherBorrowRule {
    fn name(&self) -> &'static str {
        "opinionated::test_matcher_borrow_simplification"
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

struct MatcherBorrowVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for MatcherBorrowVisitor<'_> {
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if let Some(segment) = mac.path.segments.last() {
            let mac_name = segment.ident.to_string();
            if mac_name == "assert_that" || mac_name == "expect_that" {
                self.check_assertion_macro(mac);
            }
        }

        visit::visit_macro(self, mac);
    }
}

impl MatcherBorrowVisitor<'_> {
    fn check_assertion_macro(&mut self, mac: &syn::Macro) {
        let parser = Punctuated::<syn::Expr, Token![,]>::parse_terminated;
        if let Ok(exprs) = syn::parse::Parser::parse2(parser, mac.tokens.clone()) {
            for expr in &exprs {
                let mut method_visitor = BorrowMethodVisitor {
                    ctx: self.ctx,
                    diagnostics: Vec::new(),
                };
                method_visitor.visit_expr(expr);
                self.diagnostics.extend(method_visitor.diagnostics);
            }
        }
    }
}

struct BorrowMethodVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for BorrowMethodVisitor<'_> {
    fn visit_expr_closure(&mut self, _closure: &'ast syn::ExprClosure) {
        // Do not traverse into closures like .map(|x| x.as_str())
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let method_name = call.method.to_string();
        if method_name == "as_str" || method_name == "as_slice" || method_name == "as_ref" {
            let span = self.ctx.to_span(call.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "opinionated::test_matcher_borrow_simplification",
                    Severity::Warning,
                    format!(
                        "Redundant '.{method_name}()' in GoogleTest assertion. GoogleTest matchers accept borrowed references directly."
                    ),
                )
                .with_span(span)
                .with_suggested_fix(format!(
                    "Replace '.{method_name}()' with a reference borrowing the expression directly ('&<expr>')."
                )),
            );
        }

        visit::visit_expr_method_call(self, call);
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
        assert_that!(
            &diag.rule,
            eq("opinionated::test_matcher_borrow_simplification")
        );
        assert_that!(&diag.message, contains_substring("Redundant '.as_str()'"));
        Ok(())
    }

    #[googletest::test]
    fn as_ref_in_matcher_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn sample_test() {
    expect_that!(actual, eq(expected.as_ref()));
}
"#;
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = TestMatcherBorrowRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            &diag.rule,
            eq("opinionated::test_matcher_borrow_simplification")
        );
        assert_that!(&diag.message, contains_substring("Redundant '.as_ref()'"));
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
