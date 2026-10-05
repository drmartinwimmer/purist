//! # Rule: purist::no_wildcard_imports
//!
//! ## What This Rule Does
//! Flags wildcard imports (`use path::*`) outside of test files, test modules, and prelude imports.
//!
//! ## Why This Rule Exists
//! Wildcard imports pollute the local namespace, obscure the provenance of imported symbols,
//! and make code harder to read and refactor. Furthermore, they risk silent breakage or shadowing
//! when upstream crates add new items in minor or patch releases. Explicit item imports ensure
//! clarity, deterministic name resolution, and maintainability.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! use std::collections::*;
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! use std::collections::{BTreeMap, HashMap};
//! ```

use super::common::TestScopeTracker;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging wildcard `*` imports outside test contexts and preludes.
pub struct NoWildcardImportsRule;

impl Rule for NoWildcardImportsRule {
    fn name(&self) -> &'static str {
        "purist::no_wildcard_imports"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = WildcardImportVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScopeTracker::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects `use` statements outside test scope for wildcard/glob imports.
struct WildcardImportVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScopeTracker,
}

impl<'ast> Visit<'ast> for WildcardImportVisitor<'_> {
    /// Tracks entry into and exit from test-scoped modules.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let prev = self.test_scope.enter_mod(&item_mod.attrs);
        visit::visit_item_mod(self, item_mod);
        self.test_scope.exit_mod(prev);
    }

    /// Recursively checks `use` trees if outside test scope.
    fn visit_item_use(&mut self, item_use: &'ast syn::ItemUse) {
        if !self.test_scope.is_in_test() {
            collect_wildcard_diagnostics(
                self.ctx,
                &item_use.tree,
                Vec::new(),
                &mut self.diagnostics,
            );
        }

        visit::visit_item_use(self, item_use);
    }
}

/// Recursively traverses a `syn::UseTree` to identify and report non-prelude glob imports.
fn collect_wildcard_diagnostics(
    ctx: &LintContext<'_>,
    tree: &syn::UseTree,
    mut path_segments: Vec<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match tree {
        syn::UseTree::Path(p) => {
            path_segments.push(p.ident.to_string());
            collect_wildcard_diagnostics(ctx, &p.tree, path_segments, diagnostics);
        }
        syn::UseTree::Glob(g) => {
            // Exempt preludes like googletest::prelude::* or std::io::prelude::*
            let is_prelude = path_segments
                .last()
                .map(|s| s == "prelude")
                .unwrap_or(false);
            if !is_prelude {
                let path_str = if path_segments.is_empty() {
                    "*".to_string()
                } else {
                    format!("{}::*", path_segments.join("::"))
                };
                let span = ctx.to_span(g.span());
                diagnostics.push(
                    Diagnostic::new(
                        "purist::no_wildcard_imports",
                        Severity::Warning,
                        format!(
                            "Avoid wildcard import '{path_str}'. Wildcard imports obscure symbol provenance and cause namespace pollution."
                        ),
                    )
                    .with_span(span)
                    .with_suggested_fix("Replace wildcard import with explicit item imports."),
                );
            }
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_wildcard_diagnostics(ctx, item, path_segments.clone(), diagnostics);
            }
        }
        syn::UseTree::Name(_) | syn::UseTree::Rename(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn wildcard_import_in_library_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "use std::collections::*;\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoWildcardImportsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_wildcard_imports"));
        assert_that!(&diag.message, contains_substring("std::collections::*"));
        Ok(())
    }

    #[googletest::test]
    fn prelude_wildcard_import_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "use googletest::prelude::*;\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoWildcardImportsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn wildcard_in_test_module_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[cfg(test)]
mod tests {
    use super::*;
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoWildcardImportsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
