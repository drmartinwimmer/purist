//! # Rule: purist::no_has_trait_prefix
//!
//! ## What This Rule Does
//! Forbids trait definitions using the `Has...` prefix (e.g. `HasContext`, `HasLogger`, `HasScope`).
//!
//! ## Why This Rule Exists
//! `Has...` trait naming is an OOP-style smell that treats traits as nominal property or getter
//! interfaces rather than behavioral capabilities. In idiomatic Rust:
//! - Traits providing scoped execution, context injection, or visitor state should use the `With...`
//!   prefix (e.g. `WithTestScope`, `WithLoggingContext`, `WithDepthScope`).
//! - Traits representing capabilities or data conversion should use standard nouns, verbs, or adjectives
//!   (e.g. `Display`, `AsRef`, `Read`, `IntoIterator`).
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! pub trait HasDepthScope {
//!     fn depth_scope(&self) -> &DepthScope;
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! pub trait WithDepthScope {
//!     fn depth_scope_mut(&mut self) -> &mut DepthScope;
//!     fn with_depth_step<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R;
//! }
//! ```

use super::common::has_suppression_attribute;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::visit::{self, Visit};

/// Rule forbidding `Has...` prefixes on trait definitions.
pub struct NoHasTraitPrefixRule;

impl Rule for NoHasTraitPrefixRule {
    fn name(&self) -> &'static str {
        "purist::no_has_trait_prefix"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = TraitNamingVisitor {
            ctx,
            diagnostics: Vec::new(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects trait definitions for `Has...` prefixes.
struct TraitNamingVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for TraitNamingVisitor<'_> {
    fn visit_item_trait(&mut self, item_trait: &'ast syn::ItemTrait) {
        if !has_suppression_attribute(&item_trait.attrs, "no_has_trait_prefix") {
            self.check_trait_ident_forbidden_has_prefix(item_trait);
        }

        visit::visit_item_trait(self, item_trait);
    }
}

impl TraitNamingVisitor<'_> {
    /// Emits a diagnostic if a trait name begins with `Has` followed by an uppercase letter or underscore.
    fn check_trait_ident_forbidden_has_prefix(&mut self, item_trait: &syn::ItemTrait) {
        let ident_str = item_trait.ident.to_string();
        if is_has_trait_name(&ident_str) {
            let span = self.ctx.to_span(item_trait.ident.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::no_has_trait_prefix",
                    Severity::Warning,
                    format!(
                        "Trait '{ident_str}' uses the 'Has...' prefix, which is an OOP-style anti-pattern in Rust. Use 'With...' for contextual capabilities or a descriptive noun/adjective."
                    ),
                )
                .with_span(span)
                .with_suggested_fix(
                    "Rename trait to 'With...' if providing scoped execution/context, or a descriptive noun/adjective.",
                ),
            );
        }
    }
}

/// Returns true if an identifier starts with `Has` followed by an uppercase letter or underscore.
fn is_has_trait_name(ident: &str) -> bool {
    if !ident.starts_with("Has") || ident.len() <= 3 {
        return false;
    }

    let fourth_char = ident.chars().nth(3);
    matches!(fourth_char, Some(c) if c.is_uppercase() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn has_prefix_trait_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub trait HasLogger {
    fn logger(&self);
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoHasTraitPrefixRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_has_trait_prefix"));
        assert_that!(diag.severity, eq(Severity::Warning));
        assert_that!(
            &diag.message,
            contains_substring("Trait 'HasLogger' uses the 'Has...' prefix")
        );
        Ok(())
    }

    #[googletest::test]
    fn has_snake_case_prefix_trait_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub trait Has_Context {
    fn ctx(&self);
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoHasTraitPrefixRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        Ok(())
    }

    #[googletest::test]
    fn with_prefix_trait_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub trait WithLogger {
    fn with_logger<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R;
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoHasTraitPrefixRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn hasher_and_hash_traits_are_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub trait Hasher {
    fn finish(&self) -> u64;
}

pub trait Hash {
    fn hash(&self);
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoHasTraitPrefixRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn suppressed_has_trait_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[expect(purist::no_has_trait_prefix, reason = "Legacy external compatibility interface")]
pub trait HasDatabase {
    fn db(&self);
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoHasTraitPrefixRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
