//! # Rule: opinionated::single_match_to_let_else
//!
//! ## What This Rule Does
//! Recommends `let ... = ... else { ... };` over two-arm `match` statements where one arm matches
//! a single variant (e.g. `Some(...)` or `Ok(...)`) and the other arm diverges (returns, breaks,
//! continues, or panics).
//!
//! ## Why This Rule Exists
//! Using `match` solely to unpack an `Option` or `Result` with an early-exit branch causes
//! unnecessary indentation ("rightward drift") and obscures the primary execution flow.
//! Using idiomatic `let ... else` keeps the happy path unnested and handles the error or early-exit
//! branch explicitly and cleanly at the top of the scope.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! let value = match optional_item {
//!     Some(v) => v,
//!     None => return,
//! };
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! let Some(value) = optional_item else {
//!     return;
//! };
//! ```

use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule recommending `let ... = ... else { ... };` over single-variant `match` with early exit.
pub struct SingleMatchToLetElseRule;

impl Rule for SingleMatchToLetElseRule {
    fn name(&self) -> &'static str {
        "opinionated::single_match_to_let_else"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = MatchToLetElseVisitor {
            ctx,
            diagnostics: Vec::new(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects `match` expressions for candidates that can be simplified with `let ... else`.
struct MatchToLetElseVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for MatchToLetElseVisitor<'_> {
    /// Inspects match expressions for candidates that can be converted to `let ... else`.
    fn visit_expr_match(&mut self, expr_match: &'ast syn::ExprMatch) {
        if let Some(diag) = check_match_to_let_else(self.ctx, expr_match) {
            self.diagnostics.push(diag);
        }

        visit::visit_expr_match(self, expr_match);
    }
}

/// Checks whether a two-arm match expression can be replaced by `let ... else`.
fn check_match_to_let_else(
    ctx: &LintContext<'_>,
    expr_match: &syn::ExprMatch,
) -> Option<Diagnostic> {
    if let [arm0, arm1] = &expr_match.arms[..]
        && arm0.guard.is_none()
        && arm1.guard.is_none()
    {
        let can_convert = if is_single_variant_pattern(&arm0.pat) && is_diverging_expr(&arm1.body) {
            let bound = extract_bound_idents(&arm1.pat);
            !arm_body_uses_idents(&arm1.body, &bound)
        } else if is_single_variant_pattern(&arm1.pat) && is_diverging_expr(&arm0.body) {
            let bound = extract_bound_idents(&arm0.pat);
            !arm_body_uses_idents(&arm0.body, &bound)
        } else {
            false
        };

        if can_convert {
            let span = ctx.to_span(expr_match.span());
            return Some(
                Diagnostic::new(
                    "opinionated::single_match_to_let_else",
                    Severity::Warning,
                    "Match expression can be simplified using idiomatic 'let ... = ... else { ... };' construct.",
                )
                .with_span(span)
                .with_suggested_fix("Replace 'match' with 'let Some(...) = expr else { ... };' to reduce indentation."),
            );
        }
    }

    None
}

/// Checks whether a pattern represents a single variant tuple struct like `Some(...)` or `Ok(...)`.
fn is_single_variant_pattern(pat: &syn::Pat) -> bool {
    match pat {
        syn::Pat::TupleStruct(ts) => ts
            .path
            .segments
            .last()
            .map(|s| s.ident == "Some" || s.ident == "Ok")
            .unwrap_or(false),
        _ => false,
    }
}

/// Determines whether an expression diverges (i.e. returns, breaks, continues, or panics).
fn is_diverging_expr(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Return(_) | syn::Expr::Break(_) | syn::Expr::Continue(_) => true,
        syn::Expr::Macro(mac) => mac.mac.path.segments.last().is_some_and(|ident| {
            matches!(
                ident.ident.to_string().as_str(),
                "panic" | "bail" | "todo" | "unreachable"
            )
        }),
        syn::Expr::Block(b) => b
            .block
            .stmts
            .last()
            .is_some_and(|stmt| matches!(stmt, syn::Stmt::Expr(e, _) if is_diverging_expr(e))),
        _ => false,
    }
}

/// Extracts all bound identifier names from a pattern.
fn extract_bound_idents(pat: &syn::Pat) -> Vec<String> {
    let mut idents = Vec::new();
    collect_pat_idents(pat, &mut idents);
    idents
}

/// Recursively collects identifiers bound by a pattern.
fn collect_pat_idents(pat: &syn::Pat, idents: &mut Vec<String>) {
    match pat {
        syn::Pat::Ident(pi) => {
            let name = pi.ident.to_string();
            if name != "None" && !name.starts_with('_') {
                idents.push(name);
            }
            if let Some((_, subpat)) = &pi.subpat {
                collect_pat_idents(subpat, idents);
            }
        }
        syn::Pat::TupleStruct(ts) => {
            for elem in &ts.elems {
                collect_pat_idents(elem, idents);
            }
        }
        syn::Pat::Struct(s) => {
            for field in &s.fields {
                collect_pat_idents(&field.pat, idents);
            }
        }
        syn::Pat::Tuple(t) => {
            for elem in &t.elems {
                collect_pat_idents(elem, idents);
            }
        }
        syn::Pat::Reference(r) => {
            collect_pat_idents(&r.pat, idents);
        }
        syn::Pat::Paren(p) => {
            collect_pat_idents(&p.pat, idents);
        }
        syn::Pat::Slice(s) => {
            for elem in &s.elems {
                collect_pat_idents(elem, idents);
            }
        }
        _ => {}
    }
}

/// Checks whether an expression body references any of the given identifiers.
fn arm_body_uses_idents(body: &syn::Expr, idents: &[String]) -> bool {
    if idents.is_empty() {
        return false;
    }
    let mut visitor = IdentUsageVisitor {
        idents,
        found: false,
    };
    visitor.visit_expr(body);
    visitor.found
}

/// Visitor that walks an expression to detect usages of specific identifier names.
struct IdentUsageVisitor<'a> {
    idents: &'a [String],
    found: bool,
}

impl<'ast> Visit<'ast> for IdentUsageVisitor<'_> {
    fn visit_ident(&mut self, i: &'ast proc_macro2::Ident) {
        if self.found {
            return;
        }
        let name = i.to_string();
        if self.idents.iter().any(|target| target == &name) {
            self.found = true;
        }
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if self.found {
            return;
        }
        for token in mac.tokens.clone() {
            for target in self.idents {
                if token_tree_contains_ident(&token, target) {
                    self.found = true;
                    return;
                }
            }
        }
        visit::visit_macro(self, mac);
    }
}

/// Checks if a proc_macro2 token tree contains or formats the specified identifier.
fn token_tree_contains_ident(tt: &proc_macro2::TokenTree, target: &str) -> bool {
    match tt {
        proc_macro2::TokenTree::Ident(i) => i == target,
        proc_macro2::TokenTree::Group(g) => {
            for inner in g.stream() {
                if token_tree_contains_ident(&inner, target) {
                    return true;
                }
            }
            false
        }
        proc_macro2::TokenTree::Literal(lit) => {
            let s = lit.to_string();
            s.contains(&format!("{{{target}"))
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
    fn match_some_none_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn parse_val(opt: Option<i32>) -> i32 {
    let val = match opt {
        Some(v) => v,
        None => return 0,
    };
    val + 1
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = SingleMatchToLetElseRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::single_match_to_let_else"));
        assert_that!(
            &diag.message,
            contains_substring("let ... = ... else { ... };")
        );
        Ok(())
    }

    #[googletest::test]
    fn match_with_multi_variant_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn parse_enum(val: MyEnum) -> i32 {
    match val {
        MyEnum::A => 1,
        MyEnum::B => 2,
        MyEnum::C => return 0,
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = SingleMatchToLetElseRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
