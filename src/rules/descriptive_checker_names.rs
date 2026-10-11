//! # Rule: purist::descriptive_checker_names
//!
//! ## What This Rule Does
//! Enforces that validation, lint inspection, and invariant checking helper functions follow
//! a structured naming scheme: `check_<subject>_<condition>` (or `validate_<subject>_<condition>`,
//! `verify_<subject>_<condition>`).
//!
//! ## Why This Rule Exists
//! In linter engines, AST validators, and domain invariant checkers, dozens of helper functions
//! are defined. Generic function names like `check_signature`, `validate_struct`, or `verify_call`
//! describe only the target AST subject without specifying the condition or invariant being verified.
//! Descriptive names like `check_fn_signature_negative_naming` or `check_clap_field_public_visibility`
//! make the code self-documenting and intent immediately clear.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! fn check_signature(&mut self, sig: &syn::Signature);
//! fn validate_item(&mut self, item: &syn::Item);
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! fn check_fn_signature_unsafe_in_test(&mut self, sig: &syn::Signature);
//! fn check_clap_field_public_visibility(&mut self, field: &syn::Field);
//! ```

use super::common::{TestScope, WithTestScope, has_suppression_attribute};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::visit::{self, Visit};
use syn::{ImplItemFn, ItemFn, ItemImpl};

/// Rule enforcing descriptive `check_<subject>_<condition>` function names.
pub struct DescriptiveCheckerNamesRule;

impl Rule for DescriptiveCheckerNamesRule {
    fn name(&self) -> &'static str {
        "purist::descriptive_checker_names"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = CheckerNamingVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScope::new(ctx.is_test_file()),
            in_trait_impl: false,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects function definitions for checker naming conventions.
#[derive(WithTestScope)]
struct CheckerNamingVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScope,
    in_trait_impl: bool,
}

impl<'ast> Visit<'ast> for CheckerNamingVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });
    }

    fn visit_item_impl(&mut self, item_impl: &'ast ItemImpl) {
        let was_in_trait_impl = self.in_trait_impl;
        self.in_trait_impl = item_impl.trait_.is_some();

        visit::visit_item_impl(self, item_impl);

        self.in_trait_impl = was_in_trait_impl;
    }

    fn visit_item_fn(&mut self, item_fn: &'ast ItemFn) {
        self.with_test_fn(&item_fn.attrs, |this| {
            if !this.test_scope.is_in_test()
                && !matches!(item_fn.vis, syn::Visibility::Public(_))
                && !has_suppression_attribute(&item_fn.attrs, "descriptive_checker_names")
            {
                this.check_fn_name_descriptive(&item_fn.sig.ident);
            }
            visit::visit_item_fn(this, item_fn);
        });
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast ImplItemFn) {
        self.with_test_fn(&impl_fn.attrs, |this| {
            if !this.test_scope.is_in_test()
                && !this.in_trait_impl
                && !matches!(impl_fn.vis, syn::Visibility::Public(_))
                && !has_suppression_attribute(&impl_fn.attrs, "descriptive_checker_names")
            {
                this.check_fn_name_descriptive(&impl_fn.sig.ident);
            }
            visit::visit_impl_item_fn(this, impl_fn);
        });
    }
}

impl CheckerNamingVisitor<'_> {
    /// Validates that checker helper functions follow `check_<subject>_<condition>`.
    fn check_fn_name_descriptive(&mut self, ident: &syn::Ident) {
        let name = ident.to_string();
        if is_vague_checker_name(&name) {
            let span = self.ctx.to_span(ident.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::descriptive_checker_names",
                    Severity::Warning,
                    format!(
                        "Function '{name}' uses a vague check name. Follow the 'check_<subject>_<condition>' naming convention (e.g. 'check_fn_signature_unsafe_in_test')."
                    ),
                )
                .with_span(span)
                .with_suggested_fix("Rename to include both the subject and the condition/violation being checked."),
            );
        }
    }
}

/// Returns true if a function name begins with `check_`, `validate_`, or `verify_` but lacks a distinct condition component.
fn is_vague_checker_name(name: &str) -> bool {
    let prefixes = &["check_", "validate_", "verify_"];
    let matched_prefix = prefixes.iter().find(|&&p| name.starts_with(p));

    let Some(&_prefix) = matched_prefix else {
        return false;
    };

    // Standard interface entrypoints are exempt
    if name == "check_file" {
        return false;
    }

    let parts: Vec<&str> = name.split('_').filter(|s| !s.is_empty()).collect();
    // A descriptive checker requires at least prefix + subject + condition (3 parts)
    parts.len() < 3
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn vague_check_signature_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
impl MyVisitor {
    fn check_signature(&mut self) {}
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = DescriptiveCheckerNamesRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::descriptive_checker_names"));
        assert_that!(diag.severity, eq(Severity::Warning));
        assert_that!(
            &diag.message,
            contains_substring("Function 'check_signature' uses a vague check name")
        );
        Ok(())
    }

    #[googletest::test]
    fn vague_validate_item_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn validate_item() {}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = DescriptiveCheckerNamesRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        Ok(())
    }

    #[googletest::test]
    fn descriptive_check_name_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
impl MyVisitor {
    fn check_fn_signature_negative_naming(&mut self) {}
    fn check_clap_field_public_visibility(&mut self) {}
    fn check_call_process_exit(&mut self) {}
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = DescriptiveCheckerNamesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn check_file_entrypoint_is_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn check_file() {}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = DescriptiveCheckerNamesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn trait_implementation_method_is_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub trait Checker {
    fn check_node(&self);
}

struct MyChecker;

impl Checker for MyChecker {
    fn check_node(&self) {}
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = DescriptiveCheckerNamesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn functions_in_test_contexts_are_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[cfg(test)]
mod tests {
    fn check_signature() {}
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = DescriptiveCheckerNamesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
