//! # Rule: `purist::max_nesting_depth`
//!
//! ## What This Rule Does
//! Enforces upper boundaries on the nesting depth of control flow structures
//! (`if`, `match`, `for`, `while`, `loop`).
//!
//! ## Why This Rule Exists
//! Deeply nested control flow structures (often called the "arrow anti-pattern" or "rightward drift")
//! severely impair code readability, maintainability, and cognitive comprehension. Deep nesting is
//! almost always an indicator that a function should be refactored into smaller helper functions,
//! restructured using guard clauses and early returns, or simplified with iterator combinators.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! fn process_items(items: Vec<Item>) {
//!     for item in items {
//!         if item.is_valid() {
//!             match item.kind() {
//!                 Kind::A => {
//!                     if item.has_subitems() {
//!                         for sub in item.subitems() { // Nesting depth 5 exceeds limit (4)
//!                             sub.run();
//!                         }
//!                     }
//!                 }
//!                 _ => {}
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! fn process_items(items: Vec<Item>) {
//!     for item in items {
//!         if item.is_valid() {
//!             process_single_item(&item);
//!         }
//!     }
//! }
//!
//! fn process_single_item(item: &Item) {
//!     if let Kind::A = item.kind() {
//!         for sub in item.subitems() {
//!             sub.run();
//!         }
//!     }
//! }
//! ```

use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Default maximum allowed control flow nesting depth.
pub const DEFAULT_MAX_NESTING_DEPTH: usize = 4;

/// Rule enforcing boundaries on control flow nesting depth.
pub struct MaxNestingDepthRule;

impl Rule for MaxNestingDepthRule {
    fn name(&self) -> &'static str {
        "purist::max_nesting_depth"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let max_depth = ctx
            .config()
            .and_then(|c| c.max_nesting_depth.max_depth)
            .unwrap_or(DEFAULT_MAX_NESTING_DEPTH);

        let mut visitor = NestingVisitor {
            ctx,
            max_depth,
            current_depth: 0,
            is_else_if: false,
            in_test_scope: false,
            diagnostics: Vec::new(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that traverses the AST and measures control flow nesting depth.
struct NestingVisitor<'a> {
    ctx: &'a LintContext<'a>,
    max_depth: usize,
    current_depth: usize,
    is_else_if: bool,
    in_test_scope: bool,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> NestingVisitor<'a> {
    fn check_depth(&mut self, span: proc_macro2::Span) {
        if !self.in_test_scope && self.current_depth == self.max_depth + 1 {
            let diag_span = self.ctx.to_span(span);
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::max_nesting_depth",
                    Severity::Warning,
                    format!(
                        "Control flow nesting depth of {} exceeds the maximum limit of {}.",
                        self.current_depth, self.max_depth
                    ),
                )
                .with_span(diag_span)
                .with_suggested_fix(
                    "Consider refactoring deeply nested logic into separate helper functions or using early returns.",
                ),
            );
        }
    }
}

impl<'ast> Visit<'ast> for NestingVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let is_test = is_cfg_test_attr(&item_mod.attrs);
        let prev_test = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }
        visit::visit_item_mod(self, item_mod);
        self.in_test_scope = prev_test;
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let is_test = has_test_attr(&item_fn.attrs);
        let prev_test = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }
        let prev_depth = self.current_depth;
        self.current_depth = 0;
        visit::visit_item_fn(self, item_fn);
        self.current_depth = prev_depth;
        self.in_test_scope = prev_test;
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        let is_test = has_test_attr(&impl_fn.attrs);
        let prev_test = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }
        let prev_depth = self.current_depth;
        self.current_depth = 0;
        visit::visit_impl_item_fn(self, impl_fn);
        self.current_depth = prev_depth;
        self.in_test_scope = prev_test;
    }

    fn visit_trait_item_fn(&mut self, trait_fn: &'ast syn::TraitItemFn) {
        let is_test = has_test_attr(&trait_fn.attrs);
        let prev_test = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }
        let prev_depth = self.current_depth;
        self.current_depth = 0;
        visit::visit_trait_item_fn(self, trait_fn);
        self.current_depth = prev_depth;
        self.in_test_scope = prev_test;
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        let prev_depth = self.current_depth;
        self.current_depth = 0;
        visit::visit_expr_closure(self, closure);
        self.current_depth = prev_depth;
    }

    fn visit_expr_if(&mut self, expr_if: &'ast syn::ExprIf) {
        let was_else_if = self.is_else_if;
        self.is_else_if = false;

        if !was_else_if {
            self.current_depth += 1;
            self.check_depth(expr_if.span());
        }

        self.visit_expr(&expr_if.cond);
        self.visit_block(&expr_if.then_branch);

        if let Some((_, else_expr)) = &expr_if.else_branch {
            if matches!(**else_expr, syn::Expr::If(_)) {
                self.is_else_if = true;
                self.visit_expr(else_expr);
                self.is_else_if = false;
            } else {
                self.visit_expr(else_expr);
            }
        }

        if !was_else_if {
            self.current_depth -= 1;
        }
    }

    fn visit_expr_match(&mut self, expr_match: &'ast syn::ExprMatch) {
        self.visit_expr(&expr_match.expr);
        for arm in &expr_match.arms {
            self.visit_pat(&arm.pat);
            self.current_depth += 1;
            self.check_depth(arm.body.span());
            self.visit_expr(&arm.body);
            self.current_depth -= 1;
        }
    }

    fn visit_expr_for_loop(&mut self, for_loop: &'ast syn::ExprForLoop) {
        self.visit_expr(&for_loop.expr);
        self.current_depth += 1;
        self.check_depth(for_loop.span());
        self.visit_block(&for_loop.body);
        self.current_depth -= 1;
    }

    fn visit_expr_while(&mut self, while_expr: &'ast syn::ExprWhile) {
        self.visit_expr(&while_expr.cond);
        self.current_depth += 1;
        self.check_depth(while_expr.span());
        self.visit_block(&while_expr.body);
        self.current_depth -= 1;
    }

    fn visit_expr_loop(&mut self, loop_expr: &'ast syn::ExprLoop) {
        self.current_depth += 1;
        self.check_depth(loop_expr.span());
        self.visit_block(&loop_expr.body);
        self.current_depth -= 1;
    }
}

/// Checks whether an attribute list includes `#[cfg(test)]`.
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

/// Checks whether an attribute list includes `#[test]` or `#[googletest::test]`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn shallow_nesting_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn process(val: Option<i32>) {
    if let Some(v) = val {
        if v > 0 {
            println!("{}", v);
        }
    }
}
"#;
        let file = syn::parse_file(source)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let rule = MaxNestingDepthRule;
        let diags = rule.check_file(&ctx, &file);
        expect_that!(&diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn deep_nesting_exceeding_limit_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn deep() {
    if a {
        for b in items {
            match c {
                Some(d) => {
                    while e {
                        if f {
                            do_something();
                        }
                    }
                }
                None => {}
            }
        }
    }
}
"#;
        let file = syn::parse_file(source)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let rule = MaxNestingDepthRule;
        let diags = rule.check_file(&ctx, &file);
        let diag = diags.first().ok_or("expected diagnostic")?;
        expect_that!(diags.len(), eq(1));
        expect_that!(&diag.rule, eq("purist::max_nesting_depth"));
        expect_that!(&diag.message, contains_substring("nesting depth of 5"));
        Ok(())
    }

    #[googletest::test]
    fn else_if_chain_does_not_artificially_increase_nesting()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn dispatch(x: i32) {
    if x == 1 {
        step_1();
    } else if x == 2 {
        step_2();
    } else if x == 3 {
        step_3();
    } else if x == 4 {
        step_4();
    } else if x == 5 {
        step_5();
    } else {
        step_other();
    }
}
"#;
        let file = syn::parse_file(source)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let rule = MaxNestingDepthRule;
        let diags = rule.check_file(&ctx, &file);
        expect_that!(&diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn cfg_test_modules_are_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[cfg(test)]
mod tests {
    fn deep_test() {
        if a {
            for b in items {
                match c {
                    Some(d) => {
                        while e {
                            if f {
                                assert!(true);
                            }
                        }
                    }
                    None => {}
                }
            }
        }
    }
}
"#;
        let file = syn::parse_file(source)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let rule = MaxNestingDepthRule;
        let diags = rule.check_file(&ctx, &file);
        expect_that!(&diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn match_with_guard_is_checked_for_depth() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn check_match(val: Option<i32>) {
    match val {
        Some(x) if x > 0 => {
            step_1();
        }
        _ => {}
    }
}
"#;
        let file = syn::parse_file(source)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let rule = MaxNestingDepthRule;
        let diags = rule.check_file(&ctx, &file);
        expect_that!(&diags, is_empty());
        Ok(())
    }
}
