//! # Rule: purist::prefer_scoped_closures
//!
//! ## What This Rule Does
//! Forbids manual paired lifecycle calls (such as `.push(...)` followed later by `.pop()`, or `.enter()`
//! followed by `.exit()`) within the same lexical block or function.
//!
//! ## Why This Rule Exists
//! Manual state cleanup pairs are vulnerable to state leakage when an intermediate operation returns
//! early via `?`, `return`, or panics. Using scoped closures (`with_*(..., |v| ...)`) or RAII guards
//! guarantees that the active scope or state is cleanly and deterministically restored upon exiting
//! the scope.
//!
//! Test code (`#[test]`, `#[cfg(test)]`) is exempt to allow unit testing of state collection primitives.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! self.scope.push(item);
//! self.process_item(item)?; // If this returns early, scope is never popped!
//! self.scope.pop();
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! self.with_scope(item, |this| {
//!     this.process_item(item)
//! })?;
//! ```

use super::common::{TestScope, WithTestScope, has_suppression_attribute};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use quote::ToTokens;
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Block, Expr, ExprMethodCall, ImplItemFn, ItemFn, Stmt};

/// Rule enforcing scoped closures over manual paired lifecycle calls.
pub struct PreferScopedClosuresRule;

impl Rule for PreferScopedClosuresRule {
    fn name(&self) -> &'static str {
        "purist::prefer_scoped_closures"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = LifecycleVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScope::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects function blocks for manual push/pop lifecycle pairs.
#[derive(WithTestScope)]
struct LifecycleVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScope,
}

impl<'ast> Visit<'ast> for LifecycleVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });
    }

    fn visit_item_fn(&mut self, item_fn: &'ast ItemFn) {
        self.with_test_fn(&item_fn.attrs, |this| {
            if !this.test_scope.is_in_test()
                && !has_suppression_attribute(&item_fn.attrs, "prefer_scoped_closures")
            {
                this.inspect_block(&item_fn.block);
            }
            visit::visit_item_fn(this, item_fn);
        });
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast ImplItemFn) {
        self.with_test_fn(&impl_fn.attrs, |this| {
            if !this.test_scope.is_in_test()
                && !has_suppression_attribute(&impl_fn.attrs, "prefer_scoped_closures")
            {
                this.inspect_block(&impl_fn.block);
            }
            visit::visit_impl_item_fn(this, impl_fn);
        });
    }
}

impl LifecycleVisitor<'_> {
    fn inspect_block(&mut self, block: &Block) {
        let mut active_pushes: HashMap<String, (&ExprMethodCall, String)> = HashMap::new();

        for stmt in &block.stmts {
            if let Stmt::Expr(expr, _) = stmt
                && let Expr::MethodCall(call) = expr
            {
                let method_name = call.method.to_string();
                let receiver_key = call.receiver.to_token_stream().to_string();

                if is_enter_lifecycle_method(&method_name) {
                    active_pushes.insert(receiver_key, (call, method_name));
                } else if is_exit_lifecycle_method(&method_name)
                    && let Some((enter_call, enter_name)) = active_pushes.remove(&receiver_key)
                {
                    let span = self.ctx.to_span(enter_call.span());
                    self.diagnostics.push(
                        Diagnostic::new(
                            "purist::prefer_scoped_closures",
                            Severity::Warning,
                            format!(
                                "Manual paired lifecycle calls ('{enter_name}' / '{method_name}') on '{receiver_key}'. Use a scoped closure ('with_...') or RAII guard to guarantee state restoration on error or early return."
                            ),
                        )
                        .with_span(span)
                        .with_suggested_fix("Replace manual push/pop with a scoped closure or RAII guard."),
                    );
                }
            }
        }
    }
}

/// Returns true if a method name represents entering a lifecycle state.
fn is_enter_lifecycle_method(name: &str) -> bool {
    name == "push"
        || name == "enter"
        || name == "acquire"
        || name.starts_with("push_")
        || name.starts_with("enter_")
}

/// Returns true if a method name represents exiting a lifecycle state.
fn is_exit_lifecycle_method(name: &str) -> bool {
    name == "pop"
        || name == "exit"
        || name == "release"
        || name.starts_with("pop_")
        || name.starts_with("exit_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn manual_push_pop_pair_in_block_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
impl MyVisitor {
    fn inspect(&mut self, item: syn::Item) {
        self.scope.push(item);
        self.do_something();
        self.scope.pop();
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PreferScopedClosuresRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::prefer_scoped_closures"));
        assert_that!(diag.severity, eq(Severity::Warning));
        assert_that!(
            &diag.message,
            contains_substring("Manual paired lifecycle calls ('push' / 'pop') on 'self . scope'")
        );
        Ok(())
    }

    #[googletest::test]
    fn manual_enter_exit_pair_in_block_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
impl MyVisitor {
    fn inspect(&mut self) {
        self.depth.enter();
        self.do_work();
        self.depth.exit();
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PreferScopedClosuresRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        Ok(())
    }

    #[googletest::test]
    fn scoped_closure_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
impl MyVisitor {
    fn inspect(&mut self) {
        self.with_scope(item, |this| {
            this.do_work();
        });
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PreferScopedClosuresRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn push_pop_in_test_context_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[cfg(test)]
mod tests {
    #[test]
    fn test_stack_push_pop() {
        let mut stack = Vec::new();
        stack.push(42);
        stack.pop();
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PreferScopedClosuresRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn suppressed_push_pop_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
impl MyVisitor {
    #[expect(purist::prefer_scoped_closures, reason = "Legacy imperative lifecycle requirement")]
    fn inspect(&mut self, item: syn::Item) {
        self.scope.push(item);
        self.do_something();
        self.scope.pop();
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PreferScopedClosuresRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
