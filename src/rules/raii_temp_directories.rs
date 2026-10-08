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

use super::common::{TestScope, WithTestScope, is_drop_trait_impl};
use crate::checkers::check_call_matches_path;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use crate::scopes::{FlagScope, run_with_scope};
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
            test_scope: TestScope::new(ctx.is_test_file()),
            drop_scope: FlagScope::new(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that tracks test contexts and Drop implementations while checking for manual directory removals.
#[derive(WithTestScope)]
struct TempDirVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScope,
    drop_scope: FlagScope,
}

impl TempDirVisitor<'_> {
    fn with_drop_scope<R>(&mut self, in_drop: bool, f: impl FnOnce(&mut Self) -> R) -> R {
        run_with_scope(
            self,
            |v| v.drop_scope.push(in_drop),
            |v| {
                v.drop_scope.pop();
            },
            f,
        )
    }
}

impl<'ast> Visit<'ast> for TempDirVisitor<'_> {
    /// Tracks entry into and exit from `#[cfg(test)]` modules.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });
    }

    /// Tracks entry into and exit from `Drop` trait implementations.
    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        let in_drop = is_drop_trait_impl(item_impl);
        self.with_drop_scope(in_drop, |this| {
            visit::visit_item_impl(this, item_impl);
        });
    }

    /// Tracks entry into and exit from `drop` method implementations.
    fn visit_impl_item_fn(&mut self, method: &'ast syn::ImplItemFn) {
        let in_drop = method.sig.ident == "drop";
        self.with_drop_scope(in_drop, |this| {
            visit::visit_impl_item_fn(this, method);
        });
    }

    /// Tracks entry into and exit from `#[test]` functions.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        self.with_test_fn(&item_fn.attrs, |this| {
            visit::visit_item_fn(this, item_fn);
        });
    }

    /// Inspects function calls for manual `fs::remove_dir_all` invocations.
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        self.check_call_remove_dir_all_in_test(call);
        visit::visit_expr_call(self, call);
    }
}

impl TempDirVisitor<'_> {
    /// Emits a diagnostic if `fs::remove_dir_all` is called manually in a test scope outside of `Drop`.
    fn check_call_remove_dir_all_in_test(&mut self, call: &syn::ExprCall) {
        if !self.test_scope.is_in_test() || self.drop_scope.is_active() {
            return;
        }

        const TARGETS: &[&[&str]] = &[&["fs", "remove_dir_all"], &["remove_dir_all"]];
        if !check_call_matches_path(call, TARGETS) {
            return;
        }

        let span = self.ctx.to_span(call.span());
        self.diagnostics.push(
            Diagnostic::new(
                "purist::raii_temp_directories",
                Severity::Warning,
                "Manual 'fs::remove_dir_all' in test context. Use an RAII temporary directory guard to guarantee cleanup on assertion failures.",
            )
            .with_span(span)
            .with_suggested_fix("Use an RAII temporary directory guard (implementing 'Drop' or via 'tempfile') instead of manual removal."),
        );
    }
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
