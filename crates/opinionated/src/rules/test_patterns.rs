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

struct TestVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_cfg_test: bool,
    current_fn_is_test: bool,
}

impl<'ast> Visit<'ast> for TestVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let is_cfg_test = item_mod.attrs.iter().any(|attr| {
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
        });

        let prev = self.in_cfg_test;
        if is_cfg_test {
            self.in_cfg_test = true;
        }

        visit::visit_item_mod(self, item_mod);
        self.in_cfg_test = prev;
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let has_test_attr = item_fn.attrs.iter().any(|attr| {
            attr.path().is_ident("test")
                || attr
                    .path()
                    .segments
                    .last()
                    .map(|s| s.ident == "test")
                    .unwrap_or(false)
        });

        let is_test = has_test_attr
            || (self.in_cfg_test && item_fn.sig.ident.to_string().starts_with("test_"));

        if is_test {
            self.check_test_name(&item_fn.sig.ident);
        }

        let prev_fn = self.current_fn_is_test;
        self.current_fn_is_test = is_test;
        visit::visit_item_fn(self, item_fn);
        self.current_fn_is_test = prev_fn;
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if self.current_fn_is_test
            && let Some(mac_ident) = mac.path.segments.last()
        {
            let name = mac_ident.ident.to_string();
            if name == "assert_eq" || name == "assert_ne" {
                let span = self.ctx.to_span(mac.path.span());
                self.diagnostics.push(
                    Diagnostic::new(
                        "opinionated::test_patterns",
                        Severity::Warning,
                        format!(
                            "Usage of '{name}!' in test. Use GoogleTest matchers ('assert_that!' or 'expect_that!') instead."
                        ),
                    )
                    .with_span(span)
                    .with_suggested_fix("Replace with 'assert_that!(actual, eq(expected))' or 'expect_that!(actual, eq(expected))'."),
                );
            }
        }
        visit::visit_macro(self, mac);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if self.current_fn_is_test && call.method == "unwrap" {
            let span = self.ctx.to_span(call.method.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "opinionated::test_patterns",
                    Severity::Warning,
                    "Avoid calling '.unwrap()' in test bodies. Propagate errors using '?' or assert with GoogleTest matchers.",
                )
                .with_span(span)
                .with_suggested_fix("Return Result<(), Box<dyn std::error::Error>> or googletest::Result<()> and use '?' instead of '.unwrap()'."),
            );
        }
        visit::visit_expr_method_call(self, call);
    }
}

impl TestVisitor<'_> {
    fn check_test_name(&mut self, ident: &syn::Ident) {
        let name = ident.to_string();
        let segments: Vec<&str> = name.split('_').filter(|s| !s.is_empty()).collect();

        // Valid test names have at least 3 parts: <verb>_<description>_<outcome>
        // E.g., parse_manifest_succeeds, test_parse_manifest_succeeds, check_empty_input_returns_error
        if segments.len() < 3 {
            let span = self.ctx.to_span(ident.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "opinionated::test_patterns",
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
