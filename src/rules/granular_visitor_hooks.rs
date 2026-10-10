//! # Rule: purist::granular_visitor_hooks
//!
//! ## What This Rule Does
//! Forbids manual iteration over collections of child AST nodes (such as struct fields, enum variants,
//! match arms, or file items) inside container visitor hooks within `syn::visit::Visit` or `VisitMut`
//! implementations, as well as in standalone AST inspection functions and lint rules.
//!
//! ## Why This Rule Exists
//! Iterating over child collections directly (such as `for field in &item_struct.fields`)
//! bypasses the visitor pattern's recursive traversal. This prevents child attributes
//! (such as `#[expect(...)]` or field visibility) from being handled uniformly,
//! breaks lexical scope tracking for child nodes, causes duplicate inspection logic across rules,
//! and misses nested definitions (e.g. structs inside modules or functions).
//! AST analyses should delegate to specialized leaf visitor hooks (`visit_field`, `visit_variant`, `visit_arm`).
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
//!
//! // Also non-compliant: manual AST looping in check_file or standalone functions
//! fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) {
//!     for item in &file.items {
//!         if let Item::Struct(item_struct) = item {
//!             for field in &item_struct.fields {
//!                 inspect_field(field);
//!             }
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
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    Expr, ExprForLoop, ExprMethodCall, FnArg, ImplItemFn, ItemFn, ItemImpl, Pat, PatTupleStruct,
    Type,
};

/// Rule enforcing granular visitor hooks over manual child collection iteration.
pub struct GranularVisitorHooksRule;

impl Rule for GranularVisitorHooksRule {
    fn name(&self) -> &'static str {
        "purist::granular_visitor_hooks"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = AstHookScanner {
            ctx,
            diagnostics: Vec::new(),
            in_visitor_impl: false,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects both visitor hooks and standalone functions for manual child AST loops.
struct AstHookScanner<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_visitor_impl: bool,
}

impl<'ast> Visit<'ast> for AstHookScanner<'_> {
    fn visit_item_impl(&mut self, item_impl: &'ast ItemImpl) {
        if has_suppression_attribute(&item_impl.attrs, "granular_visitor_hooks") {
            return;
        }

        let is_visitor = is_visit_impl(item_impl);
        let prev_visitor = self.in_visitor_impl;
        self.in_visitor_impl = is_visitor;

        visit::visit_item_impl(self, item_impl);

        self.in_visitor_impl = prev_visitor;
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast ImplItemFn) {
        if !has_suppression_attribute(&impl_fn.attrs, "granular_visitor_hooks") {
            self.inspect_fn(&impl_fn.sig, &impl_fn.block, self.in_visitor_impl);
        }
        visit::visit_impl_item_fn(self, impl_fn);
    }

    fn visit_item_fn(&mut self, item_fn: &'ast ItemFn) {
        if !has_suppression_attribute(&item_fn.attrs, "granular_visitor_hooks") {
            self.inspect_fn(&item_fn.sig, &item_fn.block, false);
        }
        visit::visit_item_fn(self, item_fn);
    }
}

impl AstHookScanner<'_> {
    fn inspect_fn(&mut self, sig: &syn::Signature, block: &syn::Block, in_visitor: bool) {
        let fn_name = sig.ident.to_string();

        if in_visitor && is_visitor_container_hook(&fn_name) {
            self.inspect_visitor_container_hook(&fn_name, block);
        } else {
            self.inspect_non_visitor_body(&fn_name, sig, block);
        }
    }

    fn inspect_visitor_container_hook(&mut self, fn_name: &str, block: &syn::Block) {
        let target_collection = match fn_name {
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
            loop_checker.visit_block(block);

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

    fn inspect_non_visitor_body(
        &mut self,
        fn_name: &str,
        sig: &syn::Signature,
        block: &syn::Block,
    ) {
        let mut loop_checker = NonVisitorAstLoopChecker::new(sig);
        loop_checker.visit_block(block);

        for (span, hook_name, description) in loop_checker.found_violations {
            let diag_span = self.ctx.to_span(span);
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::granular_visitor_hooks",
                    Severity::Warning,
                    format!(
                        "Manual iteration over {description} in '{fn_name}'. Implement a syn::visit::Visit visitor with the specialized '{hook_name}' hook instead of manual AST traversal."
                    ),
                )
                .with_span(diag_span)
                .with_suggested_fix(format!(
                    "Implement a visitor and use 'fn {hook_name}(&mut self, ...)'."
                )),
            );
        }
    }
}

/// Checks whether an implementation block implements `Visit` or `VisitMut`.
fn is_visit_impl(item_impl: &ItemImpl) -> bool {
    item_impl.trait_.as_ref().is_some_and(|(path, _)| {
        path_ends_with_ident(path, "Visit") || path_ends_with_ident(path, "VisitMut")
    })
}

/// Checks whether a function name matches a standard container visitor hook.
fn is_visitor_container_hook(name: &str) -> bool {
    matches!(
        name,
        "visit_item_struct" | "visit_item_enum" | "visit_expr_match" | "visit_file"
    )
}

/// AST inspector looking for loops or iterator pipelines over a specific child collection member.
struct ChildCollectionLoopChecker<'a> {
    target_field: &'a str,
    found_spans: Vec<proc_macro2::Span>,
}

impl<'ast> Visit<'ast> for ChildCollectionLoopChecker<'_> {
    fn visit_expr_for_loop(&mut self, for_loop: &'ast ExprForLoop) {
        if !has_suppression_attribute(&for_loop.attrs, "granular_visitor_hooks")
            && expr_references_field(&for_loop.expr, self.target_field)
        {
            self.found_spans.push(for_loop.span());
        }
        visit::visit_expr_for_loop(self, for_loop);
    }

    fn visit_expr_method_call(&mut self, method_call: &'ast ExprMethodCall) {
        let method_name = method_call.method.to_string();
        if method_name == "for_each"
            && !has_suppression_attribute(&method_call.attrs, "granular_visitor_hooks")
            && expr_references_field(&method_call.receiver, self.target_field)
        {
            self.found_spans.push(method_call.span());
        }
        visit::visit_expr_method_call(self, method_call);
    }
}

/// AST inspector looking for manual loops over AST child collections in non-visitor code.
struct NonVisitorAstLoopChecker {
    struct_bindings: HashSet<String>,
    enum_bindings: HashSet<String>,
    match_bindings: HashSet<String>,
    found_violations: Vec<(proc_macro2::Span, &'static str, &'static str)>,
}

impl NonVisitorAstLoopChecker {
    fn new(sig: &syn::Signature) -> Self {
        let mut struct_bindings = HashSet::new();
        let mut enum_bindings = HashSet::new();
        let mut match_bindings = HashSet::new();
        collect_ast_param_bindings(
            sig,
            &mut struct_bindings,
            &mut enum_bindings,
            &mut match_bindings,
        );

        Self {
            struct_bindings,
            enum_bindings,
            match_bindings,
            found_violations: Vec::new(),
        }
    }

    fn check_loop_expression(&mut self, expr: &Expr, span: proc_macro2::Span) {
        if let Some((receiver, field_name)) = extract_field_access(expr) {
            match field_name.as_str() {
                "fields" if is_ast_struct_receiver(&receiver, &self.struct_bindings) => {
                    self.found_violations
                        .push((span, "visit_field", "struct fields"));
                }
                "variants" if is_ast_enum_receiver(&receiver, &self.enum_bindings) => {
                    self.found_violations
                        .push((span, "visit_variant", "enum variants"));
                }
                "arms" if is_ast_match_receiver(&receiver, &self.match_bindings) => {
                    self.found_violations
                        .push((span, "visit_arm", "match arms"));
                }
                _ => {}
            }
        }
    }
}

impl<'ast> Visit<'ast> for NonVisitorAstLoopChecker {
    fn visit_pat_tuple_struct(&mut self, pat: &'ast PatTupleStruct) {
        if path_ends_with_ident(&pat.path, "Struct") && path_segment_contains(&pat.path, "Item") {
            for elem in &pat.elems {
                if let Pat::Ident(pi) = elem {
                    self.struct_bindings.insert(pi.ident.to_string());
                }
            }
        } else if path_ends_with_ident(&pat.path, "Enum")
            && path_segment_contains(&pat.path, "Item")
        {
            for elem in &pat.elems {
                if let Pat::Ident(pi) = elem {
                    self.enum_bindings.insert(pi.ident.to_string());
                }
            }
        } else if path_ends_with_ident(&pat.path, "Match")
            && path_segment_contains(&pat.path, "Expr")
        {
            for elem in &pat.elems {
                if let Pat::Ident(pi) = elem {
                    self.match_bindings.insert(pi.ident.to_string());
                }
            }
        }
        visit::visit_pat_tuple_struct(self, pat);
    }

    fn visit_expr_for_loop(&mut self, for_loop: &'ast ExprForLoop) {
        if !has_suppression_attribute(&for_loop.attrs, "granular_visitor_hooks") {
            self.check_loop_expression(&for_loop.expr, for_loop.span());
        }
        visit::visit_expr_for_loop(self, for_loop);
    }

    fn visit_expr_method_call(&mut self, method_call: &'ast ExprMethodCall) {
        let method_name = method_call.method.to_string();
        if method_name == "for_each"
            && !has_suppression_attribute(&method_call.attrs, "granular_visitor_hooks")
        {
            self.check_loop_expression(&method_call.receiver, method_call.span());
        }
        visit::visit_expr_method_call(self, method_call);
    }
}

/// Populates AST node bindings from function parameters.
fn collect_ast_param_bindings(
    sig: &syn::Signature,
    struct_bindings: &mut HashSet<String>,
    enum_bindings: &mut HashSet<String>,
    match_bindings: &mut HashSet<String>,
) {
    for input in &sig.inputs {
        if let FnArg::Typed(pat_type) = input
            && let Pat::Ident(pat_ident) = &*pat_type.pat
        {
            let ident_str = pat_ident.ident.to_string();
            if type_matches_ast_ident(&pat_type.ty, "ItemStruct") {
                struct_bindings.insert(ident_str);
            } else if type_matches_ast_ident(&pat_type.ty, "ItemEnum") {
                enum_bindings.insert(ident_str);
            } else if type_matches_ast_ident(&pat_type.ty, "ExprMatch") {
                match_bindings.insert(ident_str);
            }
        }
    }
}

/// Checks whether a type path ends with a specific AST item ident.
fn type_matches_ast_ident(ty: &Type, target_name: &str) -> bool {
    match ty {
        Type::Path(type_path) => path_ends_with_ident(&type_path.path, target_name),
        Type::Reference(type_ref) => type_matches_ast_ident(&type_ref.elem, target_name),
        _ => false,
    }
}

/// Checks whether any segment of a path matches the given name.
fn path_segment_contains(path: &syn::Path, segment_name: &str) -> bool {
    path.segments.iter().any(|seg| seg.ident == segment_name)
}

/// Checks whether a receiver refers to a struct AST node.
fn is_ast_struct_receiver(receiver: &str, ast_bindings: &HashSet<String>) -> bool {
    ast_bindings.contains(receiver)
        || receiver == "item_struct"
        || receiver == "struct_item"
        || receiver.ends_with("_struct")
        || (receiver.ends_with("struct") && receiver != "instruct")
}

/// Checks whether a receiver refers to an enum AST node.
fn is_ast_enum_receiver(receiver: &str, ast_bindings: &HashSet<String>) -> bool {
    ast_bindings.contains(receiver)
        || receiver == "item_enum"
        || receiver == "enum_item"
        || receiver.ends_with("_enum")
        || receiver.ends_with("enum")
}

/// Checks whether a receiver refers to a match AST node.
fn is_ast_match_receiver(receiver: &str, ast_bindings: &HashSet<String>) -> bool {
    ast_bindings.contains(receiver)
        || receiver == "expr_match"
        || receiver == "match_expr"
        || receiver.ends_with("_match")
        || (receiver.ends_with("match") && receiver != "mismatch")
}

/// Extracts the receiver ident and accessed field name if of the form `receiver.field`.
fn extract_field_access(expr: &Expr) -> Option<(String, String)> {
    match expr {
        Expr::Field(field_expr) => {
            if let syn::Member::Named(field_ident) = &field_expr.member
                && let Some(receiver_ident) = extract_root_ident(&field_expr.base)
            {
                Some((receiver_ident, field_ident.to_string()))
            } else {
                None
            }
        }
        Expr::Reference(ref_expr) => extract_field_access(&ref_expr.expr),
        Expr::Paren(paren_expr) => extract_field_access(&paren_expr.expr),
        Expr::MethodCall(call) => {
            let name = call.method.to_string();
            if name == "iter" || name == "into_iter" {
                extract_field_access(&call.receiver)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Recursively extracts the identifier from a simple path or field base expression.
fn extract_root_ident(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path_expr) => path_expr.path.get_ident().map(|id| id.to_string()),
        Expr::Field(field_expr) => {
            if let syn::Member::Named(id) = &field_expr.member {
                Some(id.to_string())
            } else {
                None
            }
        }
        Expr::Reference(ref_expr) => extract_root_ident(&ref_expr.expr),
        Expr::Paren(paren_expr) => extract_root_ident(&paren_expr.expr),
        _ => None,
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

    #[googletest::test]
    fn manual_struct_fields_loop_in_check_file_is_flagged() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = r#"
use syn::Item;

struct CustomRule;

impl CustomRule {
    fn check_file(&self, file: &syn::File) {
        for item in &file.items {
            if let Item::Struct(item_struct) = item {
                for field in &item_struct.fields {
                    println!("field");
                }
            }
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
            contains_substring("Manual iteration over struct fields in 'check_file'")
        );
        Ok(())
    }

    #[googletest::test]
    fn manual_enum_variants_loop_in_helper_fn_is_flagged() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = r#"
fn validate_enum_variants(item_enum: &syn::ItemEnum) {
    for variant in &item_enum.variants {
        println!("variant");
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GranularVisitorHooksRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            &diag.message,
            contains_substring("Manual iteration over enum variants in 'validate_enum_variants'")
        );
        Ok(())
    }

    #[googletest::test]
    fn manual_match_arms_loop_in_helper_fn_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn validate_match_arms(expr_match: &syn::ExprMatch) {
    for arm in &expr_match.arms {
        println!("arm");
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GranularVisitorHooksRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            &diag.message,
            contains_substring("Manual iteration over match arms in 'validate_match_arms'")
        );
        Ok(())
    }

    #[googletest::test]
    fn for_each_over_struct_fields_in_non_visitor_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn inspect_struct(item_struct: &syn::ItemStruct) {
    item_struct.fields.iter().for_each(|f| {
        println!("field");
    });
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = GranularVisitorHooksRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        Ok(())
    }

    #[googletest::test]
    fn loop_over_non_ast_struct_fields_in_function_is_permitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
struct Canvas {
    fields: Vec<String>,
}

fn render(canvas: &Canvas) {
    for field in &canvas.fields {
        println!("{field}");
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
    fn suppressed_non_visitor_loop_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[expect(purist::granular_visitor_hooks, reason = "Legacy non-visitor traversal")]
fn inspect_struct(item_struct: &syn::ItemStruct) {
    for field in &item_struct.fields {
        println!("field");
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
