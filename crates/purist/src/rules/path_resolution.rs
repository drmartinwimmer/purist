//! # Rule: purist::path_resolution
//!
//! ## What This Rule Does
//! Flags unanchored relative path literals passed to standard library filesystem operations
//! (`Path::new`, `PathBuf::from`, `fs::read`, `File::open`, etc.) or `.join(...)` calls
//! outside of test code.
//!
//! ## Why This Rule Exists
//! Unanchored relative paths (e.g. `Path::new("data/file.json")`) implicitly rely on the current
//! working directory of the process at runtime. If the binary is invoked from a different directory,
//! monorepo root, or inside a CI runner, file resolution silently fails. Production code must anchor
//! paths explicitly, such as using `Path::new(env!("CARGO_MANIFEST_DIR"))` or via an explicit
//! configuration/workspace root.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! let file = std::fs::read_to_string("config/settings.toml")?;
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
//! let file = std::fs::read_to_string(root.join("config/settings.toml"))?;
//! ```

use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::visit::{self, Visit};

/// Rule detecting unanchored relative path operations in non-test code.
pub struct PathResolutionRule;

impl Rule for PathResolutionRule {
    fn name(&self) -> &'static str {
        "purist::path_resolution"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut visitor = PathVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_test_scope: false,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor tracking test scopes and checking path constructor and filesystem calls.
struct PathVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_test_scope: bool,
}

impl<'ast> Visit<'ast> for PathVisitor<'_> {
    /// Tracks entry into and exit from `#[cfg(test)]` modules.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let is_test = is_cfg_test_attr(&item_mod.attrs);

        let previous_test_scope = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }

        visit::visit_item_mod(self, item_mod);
        self.in_test_scope = previous_test_scope;
    }

    /// Tracks entry into and exit from `#[test]` functions.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let is_test = has_test_attr(&item_fn.attrs);

        let previous_test_scope = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }

        visit::visit_item_fn(self, item_fn);
        self.in_test_scope = previous_test_scope;
    }

    /// Inspects function calls (such as `Path::new`, `File::open`) for unanchored literals.
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if !self.in_test_scope
            && let Some(diag) = check_call_for_unanchored_path(self.ctx, &call.func, &call.args)
        {
            self.diagnostics.push(diag);
        }
        visit::visit_expr_call(self, call);
    }

    /// Inspects method calls (such as `.join(...)`) for unanchored literals.
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if !self.in_test_scope
            && let Some(diag) = check_method_call_for_unanchored_path(self.ctx, call)
        {
            self.diagnostics.push(diag);
        }
        visit::visit_expr_method_call(self, call);
    }
}

/// Checks whether attributes include a `#[cfg(test)]` attribute.
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

/// Checks if a call expression is a targeted path or fs call with an unanchored path literal.
fn check_call_for_unanchored_path(
    ctx: &LintContext<'_>,
    func: &syn::Expr,
    args: &Punctuated<syn::Expr, Comma>,
) -> Option<Diagnostic> {
    let path = match func {
        syn::Expr::Path(expr_path) => &expr_path.path,
        _ => return None,
    };

    let path_str = path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::");

    let is_target_call = path_str == "Path::new"
        || path_str == "std::path::Path::new"
        || path_str == "PathBuf::from"
        || path_str == "std::path::PathBuf::from"
        || path_str == "fs::read"
        || path_str == "std::fs::read"
        || path_str == "fs::read_to_string"
        || path_str == "std::fs::read_to_string"
        || path_str == "fs::write"
        || path_str == "std::fs::write"
        || path_str == "File::open"
        || path_str == "std::fs::File::open"
        || path_str == "File::create"
        || path_str == "std::fs::File::create";

    if !is_target_call {
        return None;
    }

    let first_arg = args.first()?;
    check_expr_for_unanchored_path(ctx, first_arg)
}

/// Checks if a method call (e.g. `join`) has an unanchored path receiver.
fn check_method_call_for_unanchored_path(
    ctx: &LintContext<'_>,
    call: &syn::ExprMethodCall,
) -> Option<Diagnostic> {
    if call.method == "join" {
        check_expr_for_unanchored_path(ctx, &call.receiver)
    } else {
        None
    }
}

/// Checks whether an expression is a string literal containing an unanchored relative path.
fn check_expr_for_unanchored_path(ctx: &LintContext<'_>, expr: &syn::Expr) -> Option<Diagnostic> {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(lit_str),
        ..
    }) = expr
    {
        let val = lit_str.value();
        if is_unanchored_relative_path(&val) {
            let span = ctx.to_span(lit_str.span());
            return Some(
                Diagnostic::new(
                    "purist::path_resolution",
                    Severity::Warning,
                    format!(
                        "Unanchored relative path '{val}' in '{}'. Relative paths break when executed outside the crate root.",
                        ctx.file_path().display()
                    ),
                )
                .with_span(span)
                .with_suggested_fix(format!(
                    "Anchor path using 'Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"{val}\")' or workspace root."
                )),
            );
        }
    }
    None
}

/// Checks whether a string literal is an unanchored relative path.
fn is_unanchored_relative_path(path_str: &str) -> bool {
    let trimmed = path_str.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Absolute Unix / Windows paths
    if trimmed.starts_with('/') || trimmed.starts_with('\\') {
        return false;
    }
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2
        && bytes.first().is_some_and(|b| b.is_ascii_alphabetic())
        && bytes.get(1) == Some(&b':')
    {
        return false;
    }

    // Protocol URLs
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("file://")
    {
        return false;
    }

    // Command-line flags
    if trimmed.starts_with('-') {
        return false;
    }

    // Must look like a path or file
    trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.ends_with(".json")
        || trimmed.ends_with(".toml")
        || trimmed.ends_with(".yaml")
        || trimmed.ends_with(".txt")
        || trimmed.ends_with(".rs")
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn relative_path_new_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"fn run() { let p = std::path::Path::new("config/settings.json"); }"#;
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PathResolutionRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::path_resolution"));
        assert_that!(
            &diag.message,
            contains_substring("Unanchored relative path")
        );
        Ok(())
    }

    #[googletest::test]
    fn relative_path_in_test_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn test_something() {
    let p = std::path::Path::new("tests/fixtures/foo.txt");
}
"#;
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PathResolutionRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn absolute_path_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"fn run() { let p = std::path::Path::new("/etc/config.json"); }"#;
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PathResolutionRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
