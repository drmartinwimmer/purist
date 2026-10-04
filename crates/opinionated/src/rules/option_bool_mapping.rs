use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging verbose `if let Some(...) = ... else { false }` or `{ None }` in favor of combinators.
pub struct OptionBoolMappingRule;

impl Rule for OptionBoolMappingRule {
    fn name(&self) -> &'static str {
        "opinionated::idiomatic_option_bool_mapping"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = OptionMappingVisitor {
            ctx,
            diagnostics: Vec::new(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

struct OptionMappingVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

enum ElseKind {
    False,
    NoneVal,
    Other,
}

impl<'ast> Visit<'ast> for OptionMappingVisitor<'_> {
    fn visit_expr_if(&mut self, expr_if: &'ast syn::ExprIf) {
        if let syn::Expr::Let(expr_let) = &*expr_if.cond
            && is_pattern_some(&expr_let.pat)
            && let Some((_, else_box)) = &expr_if.else_branch
        {
            let else_kind = inspect_else_kind(else_box);
            match else_kind {
                ElseKind::False => {
                    let span = self.ctx.to_span(expr_if.span());
                    self.diagnostics.push(
                        Diagnostic::new(
                            "opinionated::idiomatic_option_bool_mapping",
                            Severity::Warning,
                            "Manual 'if let Some(...) = ... else { false }' construct. Use '.is_some_and(...)' instead.",
                        )
                        .with_span(span)
                        .with_suggested_fix("Replace with '.is_some_and(|val| ...)'."),
                    );
                }
                ElseKind::NoneVal => {
                    let span = self.ctx.to_span(expr_if.span());
                    self.diagnostics.push(
                        Diagnostic::new(
                            "opinionated::idiomatic_option_bool_mapping",
                            Severity::Warning,
                            "Manual 'if let Some(...) = ... else { None }' construct. Use functional combinators like '.and_then(...)' or '.map(...)'.",
                        )
                        .with_span(span)
                        .with_suggested_fix("Replace with '.and_then(...)' or '.map(...)'. "),
                    );
                }
                ElseKind::Other => {}
            }
        }

        visit::visit_expr_if(self, expr_if);
    }
}

fn is_pattern_some(pat: &syn::Pat) -> bool {
    match pat {
        syn::Pat::TupleStruct(ts) => ts
            .path
            .segments
            .last()
            .map(|s| s.ident == "Some")
            .unwrap_or(false),
        _ => false,
    }
}

fn inspect_else_kind(else_expr: &syn::Expr) -> ElseKind {
    match else_expr {
        syn::Expr::Block(expr_block) => {
            if expr_block.block.stmts.len() != 1 {
                return ElseKind::Other;
            }
            if let Some(stmt) = expr_block.block.stmts.first() {
                match stmt {
                    syn::Stmt::Expr(e, _) => check_single_expr(e),
                    _ => ElseKind::Other,
                }
            } else {
                ElseKind::Other
            }
        }
        _ => check_single_expr(else_expr),
    }
}

fn check_single_expr(expr: &syn::Expr) -> ElseKind {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Bool(b),
            ..
        }) => {
            if !b.value {
                ElseKind::False
            } else {
                ElseKind::Other
            }
        }
        syn::Expr::Path(p) => {
            if p.path.is_ident("None")
                || p.path
                    .segments
                    .last()
                    .map(|s| s.ident == "None")
                    .unwrap_or(false)
            {
                ElseKind::NoneVal
            } else {
                ElseKind::Other
            }
        }
        _ => ElseKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn if_let_some_else_false_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn check(val: Option<i32>) -> bool {
    if let Some(x) = val {
        x > 10
    } else {
        false
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = OptionBoolMappingRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::idiomatic_option_bool_mapping"));
        assert_that!(
            &diag.message,
            contains_substring("Manual 'if let Some(...) = ... else { false }' construct")
        );
        Ok(())
    }

    #[googletest::test]
    fn if_let_some_else_none_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn transform(val: Option<i32>) -> Option<i32> {
    if let Some(x) = val {
        Some(x * 2)
    } else {
        None
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = OptionBoolMappingRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::idiomatic_option_bool_mapping"));
        assert_that!(
            &diag.message,
            contains_substring("Manual 'if let Some(...) = ... else { None }' construct")
        );
        Ok(())
    }

    #[googletest::test]
    fn if_let_some_with_side_effect_in_else_is_permitted() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = r#"
pub fn log_and_check(val: Option<i32>) -> bool {
    if let Some(x) = val {
        x > 10
    } else {
        println!("Value was none");
        false
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = OptionBoolMappingRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
