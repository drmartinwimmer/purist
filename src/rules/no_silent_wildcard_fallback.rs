//! Rule: `purist::no_silent_wildcard_fallback`
//!
//! # What This Rule Does
//! Flags `match` expressions over mutually exclusive `Option` tuples (such as `(Some(..), None)`
//! and `(None, Some(..))`) that use a wildcard (`_`) or catch-all arm to silently return an empty
//! or default value (e.g., `String::new()`, `Vec::new()`, `Default::default()`, `""`) instead of
//! returning an explicit error or diverging.
//!
//! # Why This Rule Exists
//! When matching a tuple of mutually exclusive `Option` inputs (such as `(number, file)`), the
//! remaining states `(None, None)` and `(Some(_), Some(_))` represent missing or conflicting
//! inputs. Silently collapsing those states into `_ => String::new()` or `_ => Default::default()`
//! fabricates a bogus empty value and masks invalid state transitions or causes confusing
//! downstream failures. Each invalid state should be matched explicitly and return a descriptive
//! domain error.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! let raw_target = match (number, file) {
//!     (Some(num), None) => num,
//!     (None, Some(path)) => std::fs::read_to_string(path)?,
//!     _ => String::new(),
//! };
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! let raw_target = match (number, file) {
//!     (Some(num), None) => num,
//!     (None, Some(path)) => std::fs::read_to_string(path)?,
//!     (None, None) => return Err(CliError::MissingTarget),
//!     (Some(_), Some(_)) => return Err(CliError::ConflictingTargets),
//! };
//! ```

use super::common::{
    TestScope, WithTestScope, has_suppression_attribute, macro_name, path_ends_with_ident,
};
use crate::checkers::check_call_matches_path;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Arm, Expr, ExprMatch, Lit, Pat, Stmt};

/// Rule flagging wildcard or catch-all match arms on mutually exclusive `Option` tuples that silently return empty/default values.
pub(crate) struct NoSilentWildcardFallbackRule;

impl Rule for NoSilentWildcardFallbackRule {
    fn name(&self) -> &'static str {
        "purist::no_silent_wildcard_fallback"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut visitor = NoSilentWildcardFallbackVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScope::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects `match` expressions outside test scopes for silent wildcard fallbacks on `Option` tuples.
#[derive(WithTestScope)]
struct NoSilentWildcardFallbackVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScope,
}

impl<'ast> Visit<'ast> for NoSilentWildcardFallbackVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        self.with_test_fn(&item_fn.attrs, |this| {
            visit::visit_item_fn(this, item_fn);
        });
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        self.with_test_fn(&impl_fn.attrs, |this| {
            visit::visit_impl_item_fn(this, impl_fn);
        });
    }

    fn visit_expr_match(&mut self, expr_match: &'ast ExprMatch) {
        if !self.test_scope.is_in_test()
            && !has_suppression_attribute(&expr_match.attrs, "no_silent_wildcard_fallback")
        {
            self.check_match_silent_wildcard_fallback(expr_match);
        }

        visit::visit_expr_match(self, expr_match);
    }
}

impl NoSilentWildcardFallbackVisitor<'_> {
    fn check_match_silent_wildcard_fallback(&mut self, expr_match: &ExprMatch) {
        let mut classifier = MatchArmClassifier::default();
        visit::visit_expr_match(&mut classifier, expr_match);

        if !classifier.has_exclusive_option_tuple_arm {
            return;
        }

        for arm in classifier.silent_fallback_arms {
            if has_suppression_attribute(&arm.attrs, "no_silent_wildcard_fallback") {
                continue;
            }
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::no_silent_wildcard_fallback",
                    Severity::Warning,
                    "Match arm on mutually exclusive 'Option' tuple returns a silent empty/default fallback value instead of handling invalid states explicitly.",
                )
                .with_span(self.ctx.to_span(arm.span()))
                .with_suggested_fix(
                    "Match '(None, None)' and '(Some(_), Some(_))' explicitly and return a structured error instead of a silent empty default.",
                ),
            );
        }
    }
}

/// Helper visitor that classifies direct arms of a `match` expression without descending into nested `match` blocks.
#[derive(Default)]
struct MatchArmClassifier<'ast> {
    has_exclusive_option_tuple_arm: bool,
    silent_fallback_arms: Vec<&'ast Arm>,
}

impl<'ast> Visit<'ast> for MatchArmClassifier<'ast> {
    fn visit_expr_match(&mut self, _node: &'ast ExprMatch) {
        // Do not descend into nested match expressions.
    }

    fn visit_arm(&mut self, arm: &'ast Arm) {
        if is_exclusive_option_tuple_pat(&arm.pat) {
            self.has_exclusive_option_tuple_arm = true;
        } else if is_wildcard_or_invalid_option_tuple_pat(&arm.pat)
            && is_silent_empty_fallback_expr(&arm.body)
        {
            self.silent_fallback_arms.push(arm);
        }
    }
}

/// Returns true if `pat` is a tuple pattern mixing `Some(..)` and `None` elements (e.g., `(Some(a), None)`).
fn is_exclusive_option_tuple_pat(pat: &Pat) -> bool {
    let Pat::Tuple(pat_tuple) = pat else {
        return false;
    };
    if pat_tuple.elems.len() < 2 {
        return false;
    }

    let mut has_some = false;
    let mut has_none = false;
    for elem in &pat_tuple.elems {
        if is_some_pat(elem) {
            has_some = true;
        } else if is_none_pat(elem) {
            has_none = true;
        } else {
            return false;
        }
    }

    has_some && has_none
}

/// Returns true if `pat` is `_` or an all-`None` / all-`Some` tuple pattern (`(None, None)`, `(Some(_), Some(_))`).
fn is_wildcard_or_invalid_option_tuple_pat(pat: &Pat) -> bool {
    match pat {
        Pat::Wild(_) => true,
        Pat::Tuple(pat_tuple) if pat_tuple.elems.len() >= 2 => {
            pat_tuple.elems.iter().all(is_none_pat) || pat_tuple.elems.iter().all(is_some_pat)
        }
        _ => false,
    }
}

/// Returns true if `pat` matches `Some(..)`.
fn is_some_pat(pat: &Pat) -> bool {
    matches!(pat, Pat::TupleStruct(ts) if path_ends_with_ident(&ts.path, "Some"))
}

/// Returns true if `pat` matches `None`.
fn is_none_pat(pat: &Pat) -> bool {
    match pat {
        Pat::Ident(pi) => pi.ident == "None" && pi.subpat.is_none(),
        Pat::Path(pp) => path_ends_with_ident(&pp.path, "None"),
        _ => false,
    }
}

/// Returns true if `expr` evaluates to an empty/default fallback value (`String::new()`, `Vec::new()`, `Default::default()`, `""`, `vec![]`).
fn is_silent_empty_fallback_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Block(b) => {
            if let [Stmt::Expr(inner, None)] = b.block.stmts.as_slice() {
                is_silent_empty_fallback_expr(inner)
            } else {
                false
            }
        }
        Expr::Call(call) => {
            const EMPTY_CONSTRUCTORS: &[&[&str]] = &[
                &["String", "new"],
                &["Vec", "new"],
                &["PathBuf", "new"],
                &["Default", "default"],
            ];
            call.args.is_empty() && check_call_matches_path(call, EMPTY_CONSTRUCTORS)
        }
        Expr::Lit(expr_lit) => matches!(&expr_lit.lit, Lit::Str(s) if s.value().is_empty()),
        Expr::Macro(mac) => {
            macro_name(&mac.mac).is_some_and(|id| id == "vec") && mac.mac.tokens.is_empty()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn wildcard_string_new_on_option_tuple_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn run_factor(number: Option<String>, file: Option<String>) -> Result<String, MyError> {
    let raw = match (number, file) {
        (Some(num), None) => num,
        (None, Some(path)) => path,
        _ => String::new(),
    };
    Ok(raw)
}
"#;
        let ctx = LintContext::new(Path::new("src/cli.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoSilentWildcardFallbackRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_silent_wildcard_fallback"));
        assert_that!(
            &diag.message,
            contains_substring("returns a silent empty/default fallback value")
        );
        Ok(())
    }

    #[googletest::test]
    fn explicit_error_arms_on_option_tuple_are_permitted() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = r#"
pub fn run_factor(number: Option<String>, file: Option<String>) -> Result<String, MyError> {
    let raw = match (number, file) {
        (Some(num), None) => num,
        (None, Some(path)) => path,
        (None, None) => return Err(MyError::MissingTarget),
        (Some(_), Some(_)) => return Err(MyError::ConflictingTargets),
    };
    Ok(raw)
}
"#;
        let ctx = LintContext::new(Path::new("src/cli.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoSilentWildcardFallbackRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
