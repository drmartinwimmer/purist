//! # Rule: purist::raii_temp_directories
//!
//! ## What This Rule Does
//! Flags manual `fs::remove_dir_all` calls in test scopes (such as unit test functions or
//! `#[cfg(test)]` modules), requiring the use of RAII cleanup guards instead.
//!
//! ## Why This Rule Exists
//! Manual directory cleanup at the conclusion of a test function is fragile. If an assertion
//! fails, a panic occurs, or an early return is triggered, the cleanup line is skipped, leaving
//! temporary directories behind on disk. Using RAII temporary directory guards (such as
//! `tempfile::TempDir` or a custom `Drop` struct) guarantees that directories are cleaned up
//! on unwinding as well as normal exit.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! #[test]
//! fn test_write() {
//!     let dir = PathBuf::from("/tmp/test_dir");
//!     // ... test logic ...
//!     std::fs::remove_dir_all(&dir).unwrap();
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! #[test]
//! fn test_write() {
//!     let dir = tempfile::tempdir().unwrap();
//!     // Automatically removed on Drop, even if assertions panic
//! }
//! ```

use super::common::{
    TestScopeTracker, is_drop_trait_impl, path_ends_with_ident, path_ends_with_segments,
};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging manual `fs::remove_dir_all` in test contexts in favor of RAII cleanup.
pub struct RaiiTempDirectoriesRule;

impl Rule for RaiiTempDirectoriesRule {
    fn name(&self) -> &'static str {
        "purist::raii_temp_directories"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = TempDirVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScopeTracker::new(ctx.is_test_file()),
            in_drop_scope: false,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that tracks test contexts and Drop implementations while checking for manual directory removals.
struct TempDirVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScopeTracker,
    in_drop_scope: bool,
}

impl<'ast> Visit<'ast> for TempDirVisitor<'_> {
    /// Tracks entry into and exit from `#[cfg(test)]` modules.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let prev = self.test_scope.enter_mod(&item_mod.attrs);
        visit::visit_item_mod(self, item_mod);
        self.test_scope.exit_mod(prev);
    }

    /// Tracks entry into and exit from `Drop` trait implementations.
    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        let is_drop = is_drop_trait_impl(item_impl);

        let prev = self.in_drop_scope;
        if is_drop {
            self.in_drop_scope = true;
        }

        visit::visit_item_impl(self, item_impl);
        self.in_drop_scope = prev;
    }

    /// Tracks entry into and exit from `drop` method implementations.
    fn visit_impl_item_fn(&mut self, method: &'ast syn::ImplItemFn) {
        let is_drop_fn = method.sig.ident == "drop";
        let prev = self.in_drop_scope;
        if is_drop_fn {
            self.in_drop_scope = true;
        }

        visit::visit_impl_item_fn(self, method);
        self.in_drop_scope = prev;
    }

    /// Tracks entry into and exit from `#[test]` functions.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let prev = self.test_scope.enter_fn(&item_fn.attrs);
        visit::visit_item_fn(self, item_fn);
        self.test_scope.exit_fn(prev);
    }

    /// Inspects function calls for manual `fs::remove_dir_all` invocations.
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let Some(diag) = check_remove_dir_all_call(
            self.ctx,
            call,
            self.test_scope.is_in_test(),
            self.in_drop_scope,
        ) {
            self.diagnostics.push(diag);
        }

        visit::visit_expr_call(self, call);
    }
}

/// Emits a diagnostic if `fs::remove_dir_all` is called manually in a test scope outside of `Drop`.
fn check_remove_dir_all_call(
    ctx: &LintContext<'_>,
    call: &syn::ExprCall,
    in_test_scope: bool,
    in_drop_scope: bool,
) -> Option<Diagnostic> {
    if !in_test_scope || in_drop_scope {
        return None;
    }

    if let syn::Expr::Path(expr_path) = &*call.func
        && is_remove_dir_all_path(&expr_path.path)
    {
        let span = ctx.to_span(call.span());
        Some(
            Diagnostic::new(
                "purist::raii_temp_directories",
                Severity::Warning,
                "Manual 'fs::remove_dir_all' in test context. Use an RAII temporary directory guard to guarantee cleanup on assertion failures.",
            )
            .with_span(span)
            .with_suggested_fix("Use an RAII temporary directory guard (implementing 'Drop' or via 'tempfile') instead of manual removal."),
        )
    } else {
        None
    }
}

/// Checks whether a path refers to `remove_dir_all` or `fs::remove_dir_all`.
fn is_remove_dir_all_path(path: &syn::Path) -> bool {
    path_ends_with_segments(path, &["fs", "remove_dir_all"])
        || (path.segments.len() == 1 && path_ends_with_ident(path, "remove_dir_all"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn remove_dir_all_in_test_fn_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn file_creation_succeeds() {
    let temp_dir = std::path::PathBuf::from("/tmp/test_dir");
    std::fs::remove_dir_all(&temp_dir);
}
"#;
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = RaiiTempDirectoriesRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::raii_temp_directories"));
        assert_that!(
            &diag.message,
            contains_substring("Manual 'fs::remove_dir_all' in test context")
        );
        Ok(())
    }

    #[googletest::test]
    fn remove_dir_all_in_production_code_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn clean_cache(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::remove_dir_all(dir)
}
"#;
        let ctx = LintContext::new(Path::new("src/cache.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = RaiiTempDirectoriesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn remove_dir_all_in_drop_impl_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[cfg(test)]
mod tests {
    struct TempDir(std::path::PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            drop(std::fs::remove_dir_all(&self.0));
        }
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = RaiiTempDirectoriesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
