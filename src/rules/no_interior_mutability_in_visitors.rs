//! # Rule: purist::no_interior_mutability_in_visitors
//!
//! ## What This Rule Does
//! Forbids the use of interior mutability types (`RefCell<T>`, `Rc<RefCell<T>>`, `Cell<T>`,
//! `Mutex<T>`, `RwLock<T>`) within AST visitor structs (structs implementing `syn::visit::Visit`,
//! `VisitMut`, or named `*Visitor`).
//!
//! ## Why This Rule Exists
//! AST visitors in Rust are single-threaded and the `Visit` / `VisitMut` trait methods provide
//! `&mut self` access across AST traversal. Reaching for interior mutability introduces runtime
//! borrow check overhead, risk of panics, and architectural complexity where straightforward
//! mutable references or stack data structures (`Vec<T>`) are completely sufficient.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! struct LintVisitor {
//!     scopes: std::cell::RefCell<Vec<String>>,
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! struct LintVisitor {
//!     scopes: Vec<String>,
//! }
//! ```

use super::common::{extract_type_ident, has_suppression_attribute, path_ends_with_ident};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Item, ItemImpl, Type};

/// Rule forbidding interior mutability fields in AST visitor structs.
pub struct NoInteriorMutabilityInVisitorsRule;

impl Rule for NoInteriorMutabilityInVisitorsRule {
    fn name(&self) -> &'static str {
        "purist::no_interior_mutability_in_visitors"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let visitor_names = identify_visitor_struct_names(file);
        let mut scanner = VisitorFieldScanner {
            ctx,
            visitor_names,
            current_visitor_struct: None,
            diagnostics: Vec::new(),
        };
        scanner.visit_file(file);
        scanner.diagnostics
    }
}

/// Visitor that inspects fields of visitor structs for interior mutability.
struct VisitorFieldScanner<'a> {
    ctx: &'a LintContext<'a>,
    visitor_names: HashSet<String>,
    current_visitor_struct: Option<String>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for VisitorFieldScanner<'_> {
    fn visit_item_struct(&mut self, item_struct: &'ast syn::ItemStruct) {
        let struct_name = item_struct.ident.to_string();
        let is_visitor =
            self.visitor_names.contains(&struct_name) || struct_name.ends_with("Visitor");

        if is_visitor
            && !has_suppression_attribute(&item_struct.attrs, "no_interior_mutability_in_visitors")
        {
            let prev = self.current_visitor_struct.take();
            self.current_visitor_struct = Some(struct_name);
            visit::visit_item_struct(self, item_struct);
            self.current_visitor_struct = prev;
        } else {
            visit::visit_item_struct(self, item_struct);
        }
    }

    fn visit_field(&mut self, field: &'ast syn::Field) {
        if let Some(struct_name) = &self.current_visitor_struct
            && !has_suppression_attribute(&field.attrs, "no_interior_mutability_in_visitors")
            && let Some(bad_type) = find_interior_mutability_type(&field.ty)
        {
            let field_name = field
                .ident
                .as_ref()
                .map(|id| id.to_string())
                .unwrap_or_else(|| "<unnamed>".to_string());
            let span = self.ctx.to_span(field.span());

            self.diagnostics.push(
                Diagnostic::new(
                    "purist::no_interior_mutability_in_visitors",
                    Severity::Warning,
                    format!(
                        "Visitor struct '{struct_name}' declares field '{field_name}' with interior mutability ('{bad_type}'). AST visitors provide '&mut self' during single-threaded traversal; use direct mutable fields or stack types instead."
                    ),
                )
                .with_span(span)
                .with_suggested_fix("Replace interior mutability with direct mutable fields or a stack."),
            );
        }
        visit::visit_field(self, field);
    }
}

/// Identifies struct names that implement `Visit` or `VisitMut`.
fn identify_visitor_struct_names(file: &syn::File) -> HashSet<String> {
    let mut names = HashSet::new();
    for item in &file.items {
        if let Item::Impl(item_impl) = item
            && is_visit_trait_impl(item_impl)
            && let Some(ident) = extract_type_ident(&item_impl.self_ty)
        {
            names.insert(ident.to_string());
        }
    }
    names
}

/// Checks whether an implementation block implements `Visit` or `VisitMut`.
fn is_visit_trait_impl(item_impl: &ItemImpl) -> bool {
    item_impl.trait_.as_ref().is_some_and(|(path, _)| {
        path_ends_with_ident(path, "Visit") || path_ends_with_ident(path, "VisitMut")
    })
}

/// Returns the name of the interior mutability wrapper if found in the given type.
fn find_interior_mutability_type(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::Path(type_path) => {
            for segment in &type_path.path.segments {
                let ident_str = segment.ident.to_string();
                match ident_str.as_str() {
                    "RefCell" => return Some("RefCell"),
                    "Cell" => return Some("Cell"),
                    "Mutex" => return Some("Mutex"),
                    "RwLock" => return Some("RwLock"),
                    _ => {}
                }

                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments
                    && let Some(found) = find_in_type_args(args)
                {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

/// Recursively inspects angle-bracketed generic arguments for interior mutability types.
fn find_in_type_args(args: &syn::AngleBracketedGenericArguments) -> Option<&'static str> {
    for arg in &args.args {
        if let syn::GenericArgument::Type(inner_ty) = arg
            && let Some(found) = find_interior_mutability_type(inner_ty)
        {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn visitor_with_refcell_field_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use std::cell::RefCell;
struct SyntaxVisitor {
    scopes: RefCell<Vec<String>>,
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInteriorMutabilityInVisitorsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_interior_mutability_in_visitors"));
        assert_that!(diag.severity, eq(Severity::Warning));
        assert_that!(
            &diag.message,
            contains_substring(
                "Visitor struct 'SyntaxVisitor' declares field 'scopes' with interior mutability ('RefCell')"
            )
        );
        Ok(())
    }

    #[googletest::test]
    fn visitor_with_rc_refcell_field_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use std::rc::Rc;
use std::cell::RefCell;
struct MyVisitor {
    context: Rc<RefCell<Context>>,
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInteriorMutabilityInVisitorsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        Ok(())
    }

    #[googletest::test]
    fn struct_implementing_visit_with_mutex_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use std::sync::Mutex;
use syn::visit::Visit;

struct CustomChecker {
    count: Mutex<usize>,
}

impl<'ast> Visit<'ast> for CustomChecker {}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInteriorMutabilityInVisitorsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        Ok(())
    }

    #[googletest::test]
    fn visitor_with_clean_mutable_fields_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
struct SyntaxVisitor {
    scopes: Vec<String>,
    depth: usize,
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInteriorMutabilityInVisitorsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn non_visitor_struct_with_mutex_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use std::sync::Mutex;
struct DatabaseConnectionPool {
    pool: Mutex<Pool>,
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInteriorMutabilityInVisitorsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn suppressed_interior_mutability_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use std::cell::RefCell;
#[expect(purist::no_interior_mutability_in_visitors, reason = "Legacy third-party visitor interface requirement")]
struct LegacyVisitor {
    shared: RefCell<u32>,
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInteriorMutabilityInVisitorsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
