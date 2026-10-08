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

use super::common::{TestScope, WithTestScope};
use crate::checkers::check_call_matches_path;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
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
            test_scope: TestScope::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor tracking test scopes and checking path constructor and filesystem calls.
#[derive(WithTestScope)]
struct PathVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScope,
}

impl<'ast> Visit<'ast> for PathVisitor<'_> {
    /// Tracks entry into and exit from `#[cfg(test)]` modules.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });
    }

    /// Tracks entry into and exit from `#[test]` functions.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        self.with_test_fn(&item_fn.attrs, |this| {
            visit::visit_item_fn(this, item_fn);
        });
    }

    /// Inspects function calls (such as `Path::new`, `File::open`) for unanchored literals.
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if !self.test_scope.is_in_test() {
            self.check_call_unanchored_path_literal(call);
        }
        visit::visit_expr_call(self, call);
    }

    /// Inspects method calls (such as `.join(...)`) for unanchored literals.
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if !self.test_scope.is_in_test() {
            self.check_method_call_unanchored_path_literal(call);
        }
        visit::visit_expr_method_call(self, call);
    }
}

impl PathVisitor<'_> {
    /// Checks if a call expression is a targeted path or fs call with an unanchored path literal.
    fn check_call_unanchored_path_literal(&mut self, call: &syn::ExprCall) {
        const TARGETS: &[&[&str]] = &[
            &["Path", "new"],
            &["PathBuf", "from"],
            &["fs", "read"],
            &["fs", "read_to_string"],
            &["fs", "write"],
            &["File", "open"],
            &["File", "create"],
        ];

        if !check_call_matches_path(call, TARGETS) {
            return;
        }

        if let Some(first_arg) = call.args.first() {
            self.check_expr_unanchored_path_literal(first_arg);
        }
    }

    /// Checks if a method call (e.g. `join`) has an unanchored path receiver.
    fn check_method_call_unanchored_path_literal(&mut self, call: &syn::ExprMethodCall) {
        if call.method == "join" {
            self.check_expr_unanchored_path_literal(&call.receiver);
        }
    }

    /// Checks whether an expression is a string literal containing an unanchored relative path.
    fn check_expr_unanchored_path_literal(&mut self, expr: &syn::Expr) {
        if let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit_str),
            ..
        }) = expr
        {
            let val = lit_str.value();
            if is_unanchored_relative_path(&val) {
                let span = self.ctx.to_span(lit_str.span());
                self.diagnostics.push(
                    Diagnostic::new(
                        "purist::path_resolution",
                        Severity::Warning,
                        format!(
                            "Unanchored relative path '{val}' in '{}'. Relative paths break when executed outside the crate root.",
                            self.ctx.file_path().display()
                        ),
                    )
                    .with_span(span)
                    .with_suggested_fix(format!(
                        "Anchor path using 'Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"{val}\")' or workspace root."
                    )),
                );
            }
        }
    }
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
