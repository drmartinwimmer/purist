//! # Rule: opinionated::no_unsafe_in_tests
//!
//! ## What This Rule Does
//! Forbids the use of `unsafe` blocks and `unsafe fn` declarations within test code
//! (functions annotated with `#[test]` or items within `#[cfg(test)]` modules).
//!
//! ## Why This Rule Exists
//! Tests should verify system behavior strictly through safe, public abstractions. Writing
//! raw `unsafe` in test suites bypasses safety invariants, conceals defects in API encapsulation,
//! and introduces potential undefined behavior into the test runner. If `unsafe` is strictly
//! required for specific tests (such as low-level FFI integration), it must be explicitly suppressed
//! with `#[expect(opinionated::no_unsafe_in_tests, reason = "...")]`.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! #[test]
//! fn test_pointer() {
//!     let x = 42;
//!     unsafe {
//!         let ptr = &x as *const i32;
//!         let _ = *ptr;
//!     }
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! #[test]
//! fn test_safe_api() {
//!     let x = 42;
//!     assert_eq!(x, 42);
//! }
//! ```

use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule forbidding `unsafe` blocks and functions in test suites.
pub struct NoUnsafeInTestsRule;

impl Rule for NoUnsafeInTestsRule {
    fn name(&self) -> &'static str {
        "opinionated::no_unsafe_in_tests"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = UnsafeTestVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_cfg_test: ctx.is_test_file(),
            current_fn_is_test: false,
            suppressed_depth: 0,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects items and expressions in test scope for forbidden `unsafe` constructs.
struct UnsafeTestVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_cfg_test: bool,
    current_fn_is_test: bool,
    suppressed_depth: usize,
}

impl<'ast> Visit<'ast> for UnsafeTestVisitor<'_> {
    /// Tracks module-level test configuration and suppression scoping.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let is_cfg_test = is_cfg_test_attr(&item_mod.attrs);
        let suppressed = has_suppression(&item_mod.attrs);
        if suppressed {
            self.suppressed_depth += 1;
        }

        let prev = self.in_cfg_test;
        if is_cfg_test {
            self.in_cfg_test = true;
        }

        visit::visit_item_mod(self, item_mod);
        self.in_cfg_test = prev;

        if suppressed {
            self.suppressed_depth -= 1;
        }
    }

    /// Tracks function-level test attributes and checks for `unsafe fn` in test contexts.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let is_test_attr = has_test_attr(&item_fn.attrs);
        let suppressed = has_suppression(&item_fn.attrs);
        if suppressed {
            self.suppressed_depth += 1;
        }

        let prev = self.current_fn_is_test;
        if is_test_attr {
            self.current_fn_is_test = true;
        }

        let in_test = self.in_cfg_test || self.current_fn_is_test;
        if in_test
            && self.suppressed_depth == 0
            && let Some(diag) = check_unsafe_fn(self.ctx, item_fn)
        {
            self.diagnostics.push(diag);
        }

        visit::visit_item_fn(self, item_fn);
        self.current_fn_is_test = prev;

        if suppressed {
            self.suppressed_depth -= 1;
        }
    }

    /// Flags raw `unsafe` blocks when encountered within a test context.
    fn visit_expr_unsafe(&mut self, expr_unsafe: &'ast syn::ExprUnsafe) {
        let in_test = self.in_cfg_test || self.current_fn_is_test;
        if in_test && self.suppressed_depth == 0 {
            self.diagnostics
                .push(check_unsafe_block(self.ctx, expr_unsafe));
        }

        visit::visit_expr_unsafe(self, expr_unsafe);
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

/// Checks whether attributes contain an allow or expect suppression for this rule.
fn has_suppression(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("expect") && !attr.path().is_ident("allow") {
            return false;
        }
        let mut matched_rule = false;
        let _result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("no_unsafe_in_tests")
                || meta
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident == "no_unsafe_in_tests")
                    .unwrap_or(false)
            {
                matched_rule = true;
            }
            Ok(())
        });
        matched_rule
    })
}

/// Emits a diagnostic if the function has an `unsafe` qualifier.
fn check_unsafe_fn(ctx: &LintContext<'_>, item_fn: &syn::ItemFn) -> Option<Diagnostic> {
    if item_fn.sig.unsafety.is_some() {
        let span = ctx.to_span(item_fn.sig.fn_token.span());
        Some(
            Diagnostic::new(
                "opinionated::no_unsafe_in_tests",
                Severity::Error,
                format!(
                    "Function '{}' in test context is declared 'unsafe'. Tests must verify code through safe interfaces.",
                    item_fn.sig.ident
                ),
            )
            .with_span(span)
            .with_suggested_fix("Remove 'unsafe' from test code; verify behavior strictly through safe public interfaces."),
        )
    } else {
        None
    }
}

/// Emits a diagnostic for an `unsafe` block expression in test context.
fn check_unsafe_block(ctx: &LintContext<'_>, expr_unsafe: &syn::ExprUnsafe) -> Diagnostic {
    let span = ctx.to_span(expr_unsafe.unsafe_token.span());
    Diagnostic::new(
        "opinionated::no_unsafe_in_tests",
        Severity::Error,
        "Usage of 'unsafe' block in test context. Tests must exercise safe public abstractions.",
    )
    .with_span(span)
    .with_suggested_fix("Remove 'unsafe' block from test code; verify behavior strictly through safe public interfaces.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn unsafe_block_in_test_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn test_foo() {
    let x = 42;
    unsafe {
        let ptr = &x as *const i32;
        let _val = *ptr;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoUnsafeInTestsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::no_unsafe_in_tests"));
        assert_that!(diag.severity, eq(Severity::Error));
        Ok(())
    }

    #[googletest::test]
    fn suppressed_unsafe_in_test_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
#[expect(opinionated::no_unsafe_in_tests, reason = "FFI test requirement")]
fn test_ffi_boundary() {
    unsafe {
        let _ = libc::getpid();
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoUnsafeInTestsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
