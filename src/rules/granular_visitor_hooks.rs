//! # Rule: purist::granular_visitor_hooks
//!
//! ## What This Rule Does
//! Forbids manual iteration over collections of child AST nodes (such as struct fields, enum variants,
//! match arms, or file items) inside container visitor hooks within `syn::visit::Visit` or `VisitMut`
//! implementations.
//!
//! ## Why This Rule Exists
//! Iterating over child collections directly inside container visitor hooks (such as `for field in &item_struct.fields`
//! inside `visit_item_struct`) bypasses the visitor pattern's recursive traversal. This prevents
//! child attributes (such as `#[expect(...)]` or field visibility) from being handled uniformly,
//! breaks lexical scope tracking for child nodes, and causes duplicate inspection logic across rules.
//! Visitor implementations should delegate to specialized leaf hooks (`visit_field`, `visit_variant`, `visit_arm`).
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! impl<'ast> Visit<'ast> for MyVisitor {
//!     fn visit_item_struct(&mut self, item_struct: &'ast syn::ItemStruct) {
//!         for field in &item_struct.fields {
//!             self.check_field(field);
//!         }
//!     }
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! impl<'ast> Visit<'ast> for MyVisitor {
//!     fn visit_field(&mut self, field: &'ast syn::Field) {
//!         self.check_field(field);
//!         syn::visit::visit_field(self, field);
//!     }
//! }
//! ```

use super::common::{has_suppression_attribute, path_ends_with_ident};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, ExprForLoop, ExprMethodCall, ImplItem, ImplItemFn, ItemImpl};

/// Rule enforcing granular visitor hooks over manual child collection iteration.
pub struct GranularVisitorHooksRule;

impl Rule for GranularVisitorHooksRule {
    fn name(&self) -> &'static str {
        "purist::granular_visitor_hooks"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = VisitorImplScanner {
            ctx,
            diagnostics: Vec::new(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects `Visit` and `VisitMut` implementation blocks.
struct VisitorImplScanner<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for VisitorImplScanner<'_> {
    fn visit_item_impl(&mut self, item_impl: &'ast ItemImpl) {
        if is_visit_impl(item_impl)
            && !has_suppression_attribute(&item_impl.attrs, "granular_visitor_hooks")
        {
            for item in &item_impl.items {
                if let ImplItem::Fn(impl_fn) = item {
                    self.inspect_visitor_method(impl_fn);
                }
            }
        }

        visit::visit_item_impl(self, item_impl);
    }
}

impl VisitorImplScanner<'_> {
    fn inspect_visitor_method(&mut self, impl_fn: &ImplItemFn) {
        if has_suppression_attribute(&impl_fn.attrs, "granular_visitor_hooks") {
            return;
        }

        let fn_name = impl_fn.sig.ident.to_string();
        let target_collection = match fn_name.as_str() {
            "visit_item_struct" => Some(("fields", "visit_field", "struct fields")),
            "visit_item_enum" => Some(("variants", "visit_variant", "enum variants")),
            "visit_expr_match" => Some(("arms", "visit_arm", "match arms")),
            "visit_file" => Some(("items", "visit_item", "file items")),
            _ => None,
        };

        if let Some((collection_name, hook_name, description)) = target_collection {
            let mut loop_checker = ChildCollectionLoopChecker {
                target_field: collection_name,
                found_spans: Vec::new(),
            };
            loop_checker.visit_block(&impl_fn.block);

            for span in loop_checker.found_spans {
                let diag_span = self.ctx.to_span(span);
                self.diagnostics.push(
                    Diagnostic::new(
                        "purist::granular_visitor_hooks",
                        Severity::Warning,
                        format!(
                            "Manual iteration over {description} in '{fn_name}'. Implement the specialized '{hook_name}' visitor hook instead to preserve recursive traversal and attribute scoping."
                        ),
                    )
                    .with_span(diag_span)
                    .with_suggested_fix(format!(
                        "Remove the loop and implement 'fn {hook_name}(&mut self, ...)'."
                    )),
                );
            }
        }
    }
}

/// Checks whether an implementation block implements `Visit` or `VisitMut`.
fn is_visit_impl(item_impl: &ItemImpl) -> bool {
    item_impl.trait_.as_ref().is_some_and(|(path, _)| {
        path_ends_with_ident(path, "Visit") || path_ends_with_ident(path, "VisitMut")
    })
}

/// AST inspector looking for loops or iterator pipelines over a specific child collection member.
struct ChildCollectionLoopChecker<'a> {
    target_field: &'a str,
    found_spans: Vec<proc_macro2::Span>,
}

impl<'ast> Visit<'ast> for ChildCollectionLoopChecker<'_> {
    fn visit_expr_for_loop(&mut self, for_loop: &'ast ExprForLoop) {
        if expr_references_field(&for_loop.expr, self.target_field) {
            self.found_spans.push(for_loop.span());
        }
        visit::visit_expr_for_loop(self, for_loop);
    }

    fn visit_expr_method_call(&mut self, method_call: &'ast ExprMethodCall) {
        let method_name = method_call.method.to_string();
        if (method_name == "for_each" || method_name == "iter")
            && expr_references_field(&method_call.receiver, self.target_field)
        {
            self.found_spans.push(method_call.span());
        }
        visit::visit_expr_method_call(self, method_call);
    }
}

/// Checks whether an expression accesses the named target field (e.g. `expr.fields` or `&expr.fields`).
fn expr_references_field(expr: &Expr, target_field: &str) -> bool {
    match expr {
        Expr::Field(field_expr) => {
            if let syn::Member::Named(ident) = &field_expr.member {
                ident == target_field
            } else {
                false
            }
        }
        Expr::Reference(ref_expr) => expr_references_field(&ref_expr.expr, target_field),
        Expr::MethodCall(call) => {
            let name = call.method.to_string();
            if name == "iter" || name == "into_iter" {
                expr_references_field(&call.receiver, target_field)
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
    fn for_loop_over_struct_fields_in_visit_item_struct_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use syn::visit::Visit;

struct StructVisitor;

impl<'ast> Visit<'ast> for StructVisitor {
    fn visit_item_struct(&mut self, item_struct: &'ast syn::ItemStruct) {
        for field in &item_struct.fields {
            println!("field");
        }
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GranularVisitorHooksRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::granular_visitor_hooks"));
        assert_that!(diag.severity, eq(Severity::Warning));
        assert_that!(
            &diag.message,
            contains_substring("Manual iteration over struct fields in 'visit_item_struct'")
        );
        Ok(())
    }

    #[googletest::test]
    fn for_loop_over_enum_variants_in_visit_item_enum_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use syn::visit::Visit;

struct EnumVisitor;

impl<'ast> Visit<'ast> for EnumVisitor {
    fn visit_item_enum(&mut self, item_enum: &'ast syn::ItemEnum) {
        for variant in &item_enum.variants {
            println!("variant");
        }
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GranularVisitorHooksRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        Ok(())
    }

    #[googletest::test]
    fn for_loop_over_match_arms_in_visit_expr_match_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use syn::visit::Visit;

struct MatchVisitor;

impl<'ast> Visit<'ast> for MatchVisitor {
    fn visit_expr_match(&mut self, expr_match: &'ast syn::ExprMatch) {
        for arm in &expr_match.arms {
            println!("arm");
        }
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GranularVisitorHooksRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        Ok(())
    }

    #[googletest::test]
    fn granular_hook_visit_field_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use syn::visit::{self, Visit};

struct CleanVisitor;

impl<'ast> Visit<'ast> for CleanVisitor {
    fn visit_field(&mut self, field: &'ast syn::Field) {
        println!("field");
        visit::visit_field(self, field);
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GranularVisitorHooksRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn loop_in_non_visitor_struct_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
struct Formatter;

impl Formatter {
    fn format_fields(&self, fields: &[syn::Field]) {
        for field in fields {
            println!("field");
        }
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GranularVisitorHooksRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn suppressed_iteration_in_visitor_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use syn::visit::Visit;

struct SuppressedVisitor;

impl<'ast> Visit<'ast> for SuppressedVisitor {
    #[expect(purist::granular_visitor_hooks, reason = "Legacy bulk field inspection")]
    fn visit_item_struct(&mut self, item_struct: &'ast syn::ItemStruct) {
        for field in &item_struct.fields {
            println!("field");
        }
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GranularVisitorHooksRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
