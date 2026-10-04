//! # Rule: purist::use_declarations_over_qualified_paths
//!
//! ## What This Rule Does
//! Flags deeply nested, long qualified paths (such as `crate::workspace::Workspace` or
//! `syn::punctuated::Punctuated`) within type annotations and expressions, recommending
//! an explicit `use` statement at module level instead.
//!
//! ## Why This Rule Exists
//! Repeatedly typing out deep multi-segment paths creates noisy, hard-to-read function signatures
//! and expressions. Importing items at the top of the file via `use` declarations clarifies
//! the external dependencies of the module, standardizes naming, and keeps local logic concise.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! fn process(w: crate::workspace::Workspace) {}
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! use crate::workspace::Workspace;
//!
//! fn process(w: Workspace) {}
//! ```

use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule detecting verbose qualified paths in code that should be imported via `use` statements.
pub struct UseDeclarationsRule;

impl Rule for UseDeclarationsRule {
    fn name(&self) -> &'static str {
        "purist::use_declarations_over_qualified_paths"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let imported_names = collect_imported_names(file);

        let mut visitor = QualifiedPathVisitor {
            ctx,
            diagnostics: Vec::new(),
            imported_names: &imported_names,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Collects all base and renamed identifier symbols brought into scope by file-level `use` statements.
fn collect_imported_names(file: &syn::File) -> HashSet<String> {
    let mut names = HashSet::new();
    for item in &file.items {
        if let syn::Item::Use(item_use) = item {
            collect_use_tree_names(&item_use.tree, &mut names);
        }
    }
    names
}

/// Recursively traverses a `syn::UseTree` to extract imported symbol names.
fn collect_use_tree_names(tree: &syn::UseTree, names: &mut HashSet<String>) {
    match tree {
        syn::UseTree::Path(p) => {
            if is_self_use_tree(&p.tree) {
                names.insert(p.ident.to_string());
            }
            collect_use_tree_names(&p.tree, names);
        }
        syn::UseTree::Name(n) => {
            names.insert(n.ident.to_string());
        }
        syn::UseTree::Rename(r) => {
            names.insert(r.rename.to_string());
        }
        syn::UseTree::Group(g) => {
            for item in &g.items {
                collect_use_tree_names(item, names);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

/// Checks if a `use` subtree imports `self` (e.g. `use foo::{self, bar}`).
fn is_self_use_tree(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Name(n) => n.ident == "self",
        syn::UseTree::Group(g) => g.items.iter().any(is_self_use_tree),
        _ => false,
    }
}

/// Visitor that inspects type and expression paths, flagging overly long qualified references.
struct QualifiedPathVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    imported_names: &'a HashSet<String>,
}

impl<'ast> Visit<'ast> for QualifiedPathVisitor<'_> {
    /// Skips inspecting paths inside `use` statements themselves.
    fn visit_item_use(&mut self, _item_use: &'ast syn::ItemUse) {
        // Skip inspect paths inside use statements
    }

    /// Skips paths inside attribute metadata (such as `#[googletest::test]`).
    fn visit_attribute(&mut self, _attr: &'ast syn::Attribute) {
        // Skip paths inside attribute metadata (like #[googletest::test] or #[derive(...)])
    }

    /// Skips paths inside macro invocations.
    fn visit_macro(&mut self, _mac: &'ast syn::Macro) {
        // Skip paths inside macro invocations
    }

    /// Checks type paths for verbose multi-segment qualification.
    fn visit_type_path(&mut self, type_path: &'ast syn::TypePath) {
        if let Some(diag) = check_qualified_path(self.ctx, &type_path.path, self.imported_names) {
            self.diagnostics.push(diag);
        }
        visit::visit_type_path(self, type_path);
    }

    /// Checks expression paths for verbose multi-segment qualification.
    fn visit_expr_path(&mut self, expr_path: &'ast syn::ExprPath) {
        if let Some(diag) = check_qualified_path(self.ctx, &expr_path.path, self.imported_names) {
            self.diagnostics.push(diag);
        }
        visit::visit_expr_path(self, expr_path);
    }
}

/// Checks whether a path qualifies as an excessively long path that should be replaced with a `use` declaration.
fn check_qualified_path(
    ctx: &LintContext<'_>,
    path: &syn::Path,
    imported_names: &HashSet<String>,
) -> Option<Diagnostic> {
    if !should_flag_qualified_path(path, imported_names) {
        return None;
    }

    let path_str = path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::");

    let target_ident = path.segments.last()?;
    let span = ctx.to_span(path.span());
    Some(
        Diagnostic::new(
            "purist::use_declarations_over_qualified_paths",
            Severity::Warning,
            format!(
                "Avoid long qualified path '{path_str}'. Add a 'use' declaration at the top of the module."
            ),
        )
        .with_span(span)
        .with_suggested_fix(format!(
            "Import '{path_str}' via 'use {path_str};' and refer to '{}' directly.",
            target_ident.ident
        )),
    )
}

/// Checks whether a name is a Rust primitive type (e.g. `usize`).
fn is_primitive_type(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "char"
            | "str"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "f32"
            | "f64"
    )
}

/// Determines whether a qualified path should be flagged.
fn should_flag_qualified_path(path: &syn::Path, imported_names: &HashSet<String>) -> bool {
    let segments = &path.segments;
    // Long qualified paths have at least 3 segments (e.g. crate::workspace::Workspace, syn::visit::Visit)
    if segments.len() < 3 {
        return false;
    }

    let first = segments
        .first()
        .map(|s| s.ident.to_string())
        .unwrap_or_default();

    // Exempt primitive types (e.g. usize::MAX)
    if is_primitive_type(&first) {
        return false;
    }

    // Types starting with uppercase (e.g. Path::new, Vec::new, MyStruct::method) are associated items
    if first
        .chars()
        .next()
        .map(|c| c.is_ascii_uppercase())
        .unwrap_or(false)
    {
        return false;
    }

    // Exempt common standard libraries and self
    if first == "std" || first == "core" || first == "alloc" || first == "self" {
        return false;
    }

    // If the module prefix was explicitly imported via a use declaration, check remaining segments
    if imported_names.contains(&first) {
        return segments.len() >= 4;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn qualified_crate_path_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "fn process(w: crate::workspace::Workspace) {}\n";
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = UseDeclarationsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            &diag.rule,
            eq("purist::use_declarations_over_qualified_paths")
        );
        assert_that!(
            &diag.message,
            contains_substring("crate::workspace::Workspace")
        );
        Ok(())
    }

    #[googletest::test]
    fn external_crate_long_qualified_path_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "fn init() -> syn::punctuated::Punctuated { todo!() }\n";
        let ctx = LintContext::new(Path::new("src/config.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = UseDeclarationsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            &diag.rule,
            eq("purist::use_declarations_over_qualified_paths")
        );
        assert_that!(
            &diag.message,
            contains_substring("syn::punctuated::Punctuated")
        );
        Ok(())
    }

    #[googletest::test]
    fn two_segment_external_path_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "fn process(file: syn::File) -> googletest::Result<()> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = UseDeclarationsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn primitive_associated_item_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "fn max() -> usize { usize::MAX }\n";
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = UseDeclarationsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn imported_path_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "use crate::workspace::Workspace;\nfn process(w: Workspace) {}\n";
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = UseDeclarationsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn standard_library_path_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "fn open(p: &std::path::Path) {}\n";
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = UseDeclarationsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
