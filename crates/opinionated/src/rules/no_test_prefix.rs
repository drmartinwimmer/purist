use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::visit::{self, Visit};

/// Rule forbidding `test_` or `test` prefixes in test function names.
pub struct NoTestPrefixRule;

impl Rule for NoTestPrefixRule {
    fn name(&self) -> &'static str {
        "opinionated::no_test_prefix"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = TestPrefixVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_cfg_test: ctx.is_test_file(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

struct TestPrefixVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_cfg_test: bool,
}

impl<'ast> Visit<'ast> for TestPrefixVisitor<'_> {
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
        self.check_fn_name(&item_fn.sig.ident, &item_fn.attrs);
        visit::visit_item_fn(self, item_fn);
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        self.check_fn_name(&impl_fn.sig.ident, &impl_fn.attrs);
        visit::visit_impl_item_fn(self, impl_fn);
    }
}

impl TestPrefixVisitor<'_> {
    fn check_fn_name(&mut self, ident: &syn::Ident, attrs: &[syn::Attribute]) {
        let has_test_attr = attrs.iter().any(|attr| {
            attr.path().is_ident("test")
                || attr
                    .path()
                    .segments
                    .last()
                    .map(|s| s.ident == "test")
                    .unwrap_or(false)
        });

        let is_test = has_test_attr || self.in_cfg_test;
        if !is_test {
            return;
        }

        let name = ident.to_string();
        let starts_with_prefix = name.starts_with("test_")
            || name == "test"
            || (name.starts_with("test")
                && name
                    .chars()
                    .nth(4)
                    .map(|c| c.is_ascii_uppercase() || c == '_')
                    .unwrap_or(false));

        if starts_with_prefix {
            let span = self.ctx.to_span(ident.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "opinionated::no_test_prefix",
                    Severity::Warning,
                    format!(
                        "Test function '{name}' has a redundant 'test_' prefix. Use '<action>_<scenario>_<outcome>' naming (e.g. 'parse_valid_manifest_succeeds')."
                    ),
                )
                .with_span(span)
                .with_suggested_fix(format!(
                    "Rename test '{name}' to remove the 'test_' prefix and follow '<action>_<scenario>_<outcome>'."
                )),
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
    fn test_prefix_in_test_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "#[test]\nfn test_parse_manifest() {}\n";
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTestPrefixRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::no_test_prefix"));
        assert_that!(
            &diag.message,
            contains_substring("redundant 'test_' prefix")
        );
        Ok(())
    }

    #[googletest::test]
    fn descriptive_test_name_without_prefix_is_permitted() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = "#[test]\nfn parse_valid_manifest_succeeds() {}\n";
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTestPrefixRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn non_test_fn_with_test_in_name_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn test_connection() -> bool { true }\n";
        let ctx = LintContext::new(Path::new("src/network.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTestPrefixRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
