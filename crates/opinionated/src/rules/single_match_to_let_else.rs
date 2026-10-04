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

struct MatchToLetElseVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for MatchToLetElseVisitor<'_> {
    fn visit_expr_match(&mut self, expr_match: &'ast syn::ExprMatch) {
        if let [arm0, arm1] = &expr_match.arms[..] {
            let can_convert = (is_single_variant_pattern(&arm0.pat)
                && is_diverging_expr(&arm1.body))
                || (is_single_variant_pattern(&arm1.pat) && is_diverging_expr(&arm0.body));

            if can_convert {
                let span = self.ctx.to_span(expr_match.span());
                self.diagnostics.push(
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

        visit::visit_expr_match(self, expr_match);
    }
}

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

fn is_diverging_expr(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Return(_) | syn::Expr::Break(_) | syn::Expr::Continue(_) => true,
        syn::Expr::Macro(mac) => {
            if let Some(ident) = mac.mac.path.segments.last() {
                matches!(
                    ident.ident.to_string().as_str(),
                    "panic" | "bail" | "todo" | "unreachable"
                )
            } else {
                false
            }
        }
        syn::Expr::Block(b) => {
            if let Some(syn::Stmt::Expr(e, _)) = b.block.stmts.last() {
                is_diverging_expr(e)
            } else {
                false
            }
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
    fn match_some_with_early_return_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn parse(opt: Option<i32>) -> Result<i32, String> {
    let val = match opt {
        Some(x) => x,
        None => return Err("empty".to_string()),
    };
    Ok(val)
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
            contains_substring("simplified using idiomatic 'let ... = ... else")
        );
        Ok(())
    }

    #[googletest::test]
    fn multi_arm_match_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn parse_num(n: i32) -> &'static str {
    match n {
        0 => "zero",
        1 => "one",
        _ => "many",
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
