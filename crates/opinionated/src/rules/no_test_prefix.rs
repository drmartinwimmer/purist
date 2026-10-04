//! # Rule: opinionated::no_test_prefix
//!
//! ## What This Rule Does
//! Flags test functions that use a redundant `test_` or `test` prefix in their name.
//!
//! ## Why This Rule Exists
//! Test functions are already explicitly annotated with `#[test]` (or test framework macros)
//! and located within `#[cfg(test)]` modules or test files. Adding a `test_` prefix is redundant
//! noise. Test functions should instead adopt a descriptive convention describing the behavior,
//! such as `<action>_<scenario>_<outcome>` (e.g. `parse_valid_manifest_succeeds`).
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! #[test]
//! fn test_parse_manifest() {
//!     // ...
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! #[test]
//! fn parse_valid_manifest_succeeds() {
//!     // ...
//! }
//! ```

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

/// Visitor that walks modules and functions, tracking test scope and inspecting test names.
struct TestPrefixVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_cfg_test: bool,
}

impl<'ast> Visit<'ast> for TestPrefixVisitor<'_> {
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

    /// Checks top-level functions for forbidden test prefixes when in test scope.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        if let Some(diag) = check_function_name(
            self.ctx,
            &item_fn.sig.ident,
            &item_fn.attrs,
            self.in_cfg_test,
        ) {
            self.diagnostics.push(diag);
        }
        visit::visit_item_fn(self, item_fn);
    }

    /// Checks impl-level functions for forbidden test prefixes when in test scope.
    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        if let Some(diag) = check_function_name(
            self.ctx,
            &impl_fn.sig.ident,
            &impl_fn.attrs,
            self.in_cfg_test,
        ) {
            self.diagnostics.push(diag);
        }
        visit::visit_impl_item_fn(self, impl_fn);
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

/// Determines if a function name begins with a forbidden `test_` or `test` prefix.
fn has_forbidden_prefix(name: &str) -> bool {
    name.starts_with("test_")
        || name == "test"
        || (name.starts_with("test")
            && name
                .chars()
                .nth(4)
                .map(|c| c.is_ascii_uppercase() || c == '_')
                .unwrap_or(false))
}

/// Inspects a function identifier and attributes, returning a diagnostic if it has a redundant test prefix.
fn check_function_name(
    ctx: &LintContext<'_>,
    ident: &syn::Ident,
    attrs: &[syn::Attribute],
    in_cfg_test: bool,
) -> Option<Diagnostic> {
    let is_test = has_test_attr(attrs) || in_cfg_test;
    if !is_test {
        return None;
    }

    let name = ident.to_string();
    if !has_forbidden_prefix(&name) {
        return None;
    }

    let span = ctx.to_span(ident.span());
    Some(
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
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn prefix_in_test_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
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
