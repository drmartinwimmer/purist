//! # Rule: `purist::no_trivial_getters_setters`
//!
//! ## What This Rule Does
//! Identifies trivial getter and setter combos for struct fields where exposing or accessing
//! the field directly would suffice, in accordance with YAGNI (You Aren't Gonna Need It) principles.
//! Specifically flags:
//! 1. Struct fields that have both a trivial getter and a trivial setter.
//! 2. Struct fields that have both a trivial getter and a trivial mutable getter (`&mut self.field`).
//!
//! Whether the field itself is private or public, providing both a trivial getter and a trivial
//! setter adds unnecessary indirection and ceremony without invariant enforcement or data hiding.
//!
//! ## Why This Rule Exists
//! In object-oriented programming (e.g. Java), encapsulating every field behind a getter
//! and setter is boilerplate ceremony. In Rust, this pattern is often an anti-pattern:
//! - Unvalidated getters and setters provide zero invariant enforcement or data hiding.
//! - Methods taking `&self` or `&mut self` borrow the entire struct, preventing partial borrows
//!   across unrelated fields.
//! - Callers cannot pattern match or destructure the struct directly.
//! - If callers can freely read and overwrite the field without validation or side effects,
//!   writing trivial getter/setter pairs adds maintenance burden and indirection for no benefit.
//!
//! Instead, expose the field directly (`pub field: Type`) or provide encapsulation only when
//! actual validation, transformation, or invariant checking is required. Read-only getters
//! (with no setter or mutator) are permitted for encapsulation.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! pub struct Point {
//!     x: f64,
//!     y: f64,
//! }
//!
//! impl Point {
//!     pub fn x(&self) -> f64 {
//!         self.x
//!     }
//!     pub fn set_x(&mut self, x: f64) {
//!         self.x = x;
//!     }
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! pub struct Point {
//!     pub x: f64,
//!     pub y: f64,
//! }
//! ```

use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use crate::trackers::derives_clap;
use std::collections::{HashMap, HashSet};

/// Rule detecting trivial getter/setter combos where direct field access suffices.
pub struct NoTrivialGettersSettersRule;

impl Rule for NoTrivialGettersSettersRule {
    fn name(&self) -> &'static str {
        "purist::no_trivial_getters_setters"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if ctx.is_test_file() {
            return diagnostics;
        }

        let items = collect_non_test_items(file);
        let exempt_structs = collect_exempt_structs(&items);
        let methods_by_struct = collect_inherent_methods(&items);

        for (struct_name, methods) in &methods_by_struct {
            if exempt_structs.contains(struct_name) {
                continue;
            }
            check_struct_getter_setter_combos(
                ctx,
                self.name(),
                struct_name,
                methods,
                &mut diagnostics,
            );
        }

        diagnostics.sort_by_key(|d| {
            d.span
                .as_ref()
                .map(|s| (s.start_line, s.start_col))
                .unwrap_or((0, 0))
        });
        diagnostics
    }
}

/// Information about an inherent method.
struct MethodInfo<'a> {
    name: String,
    span: proc_macro2::Span,
    sig: &'a syn::Signature,
    block: &'a syn::Block,
}

#[derive(Default)]
struct FieldAccessors<'a> {
    getter: Option<&'a MethodInfo<'a>>,
    setter: Option<&'a MethodInfo<'a>>,
    mut_getter: Option<&'a MethodInfo<'a>>,
}

/// Recursively collects items from a file, skipping test modules.
fn collect_non_test_items(file: &syn::File) -> Vec<&syn::Item> {
    let mut collected = Vec::new();
    collect_items_recursive(&file.items, &mut collected);
    collected
}

fn collect_items_recursive<'a>(items: &'a [syn::Item], out: &mut Vec<&'a syn::Item>) {
    for item in items {
        if has_test_attr(get_item_attrs(item)) {
            continue;
        }
        if let syn::Item::Mod(m) = item
            && let Some((_, inner)) = &m.content
        {
            collect_items_recursive(inner, out);
        } else {
            out.push(item);
        }
    }
}

fn get_item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        _ => &[],
    }
}

fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("test")
            || a.path().segments.last().is_some_and(|s| s.ident == "test")
            || (a.path().is_ident("cfg")
                && a.parse_nested_meta(|m| {
                    if m.path.is_ident("test") {
                        Ok(())
                    } else {
                        Err(m.error(""))
                    }
                })
                .is_ok())
    })
}

fn has_rule_suppression(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        (a.path().is_ident("allow") || a.path().is_ident("expect"))
            && a.parse_nested_meta(|m| {
                let matches_name = m.path.is_ident("no_trivial_getters_setters")
                    || m.path.is_ident("no_trivial_getset")
                    || m.path.is_ident("trivial_getters_setters")
                    || m.path.is_ident("trivial_getset")
                    || m.path.segments.last().is_some_and(|s| {
                        s.ident == "no_trivial_getters_setters"
                            || s.ident == "trivial_getters_setters"
                    });
                if matches_name {
                    Err(m.error("suppressed"))
                } else {
                    Ok(())
                }
            })
            .is_err()
    })
}

/// Collects names of structs that derive Clap or have rule-level suppressions.
fn collect_exempt_structs(items: &[&syn::Item]) -> HashSet<String> {
    let mut exempt = HashSet::new();
    for item in items {
        if let syn::Item::Struct(s) = item
            && (derives_clap(&s.attrs) || has_rule_suppression(&s.attrs))
        {
            exempt.insert(s.ident.to_string());
        }
    }
    exempt
}

/// Collects inherent methods grouped by struct name.
fn collect_inherent_methods<'a>(items: &[&'a syn::Item]) -> HashMap<String, Vec<MethodInfo<'a>>> {
    let mut map: HashMap<String, Vec<MethodInfo<'a>>> = HashMap::new();
    for item in items {
        if let syn::Item::Impl(item_impl) = item
            && item_impl.trait_.is_none()
            && !has_rule_suppression(&item_impl.attrs)
            && let Some(name) = extract_type_ident(&item_impl.self_ty)
        {
            for impl_item in &item_impl.items {
                if let syn::ImplItem::Fn(f) = impl_item
                    && !is_exempt_method(f)
                {
                    map.entry(name.clone()).or_default().push(MethodInfo {
                        name: f.sig.ident.to_string(),
                        span: f.sig.ident.span(),
                        sig: &f.sig,
                        block: &f.block,
                    });
                }
            }
        }
    }
    map
}

fn extract_type_ident(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(type_path) = ty
        && type_path.qself.is_none()
    {
        type_path.path.segments.last().map(|s| s.ident.to_string())
    } else {
        None
    }
}

fn is_exempt_method(item_fn: &syn::ImplItemFn) -> bool {
    has_rule_suppression(&item_fn.attrs)
        || item_fn.attrs.iter().any(|attr| {
            attr.path().is_ident("deprecated")
                || attr.path().is_ident("test")
                || attr
                    .path()
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "test")
        })
}

struct DiagEmitter<'a, 'c> {
    ctx: &'a LintContext<'c>,
    rule_name: &'static str,
    struct_name: &'a str,
    field_name: &'a str,
    diagnostics: &'a mut Vec<Diagnostic>,
}

impl DiagEmitter<'_, '_> {
    fn emit(&mut self, method: &MethodInfo<'_>, kind: &str, partner_name: &str) {
        let msg = format!(
            "Method '{}' is a trivial {kind} for field '{}' on struct '{}' which forms a trivial getter/setter combo with '{partner_name}'. Trivial getter/setter combos add unnecessary indirection (YAGNI).",
            method.name, self.field_name, self.struct_name
        );
        let fix = format!(
            "Remove trivial {kind} '{}' and access field '{}' directly.",
            method.name, self.field_name
        );
        self.diagnostics.push(
            Diagnostic::new(self.rule_name, Severity::Warning, msg)
                .with_span(self.ctx.to_span(method.span))
                .with_suggested_fix(fix),
        );
    }
}

/// Checks methods of a struct for trivial getter/setter combos.
fn check_struct_getter_setter_combos(
    ctx: &LintContext<'_>,
    rule_name: &'static str,
    struct_name: &str,
    methods: &[MethodInfo<'_>],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut field_accessors: HashMap<String, FieldAccessors<'_>> = HashMap::new();

    for m in methods {
        if let Some(field) = detect_trivial_getter(m) {
            field_accessors.entry(field).or_default().getter = Some(m);
        }
        if let Some(field) = detect_trivial_setter(m) {
            field_accessors.entry(field).or_default().setter = Some(m);
        }
        if let Some(field) = detect_trivial_mut_getter(m) {
            field_accessors.entry(field).or_default().mut_getter = Some(m);
        }
    }

    for (field_name, accessors) in &field_accessors {
        let Some(getter) = accessors.getter else {
            continue;
        };

        let has_setter = accessors.setter.is_some();
        let has_mut_getter = accessors.mut_getter.is_some();
        if !has_setter && !has_mut_getter {
            continue;
        }

        let partner_name = accessors
            .setter
            .or(accessors.mut_getter)
            .map(|m| m.name.as_str())
            .unwrap_or("mutator");

        let mut emitter = DiagEmitter {
            ctx,
            rule_name,
            struct_name,
            field_name,
            diagnostics,
        };

        emitter.emit(getter, "getter", partner_name);

        if let Some(setter) = accessors.setter {
            emitter.emit(setter, "setter", &getter.name);
        }

        if let Some(mut_getter) = accessors.mut_getter {
            emitter.emit(mut_getter, "mutable getter", &getter.name);
        }
    }
}

fn is_receiver_ref(sig: &syn::Signature, is_mut: bool) -> bool {
    match sig.inputs.first() {
        Some(syn::FnArg::Receiver(r)) => match &r.kind {
            syn::ReceiverKind::Reference(_, _, mutability) => mutability.is_some() == is_mut,
            _ => false,
        },
        _ => false,
    }
}

fn is_self_field(expr: &syn::Expr) -> Option<&syn::Ident> {
    if let syn::Expr::Field(f) = expr
        && is_self_expr(&f.base)
        && let syn::Member::Named(ident) = &f.member
    {
        Some(ident)
    } else {
        None
    }
}

/// Detects whether `method` is a trivial getter and returns the target field name.
fn detect_trivial_getter(method: &MethodInfo<'_>) -> Option<String> {
    if method.sig.inputs.len() != 1
        || !is_receiver_ref(method.sig, false)
        || !matches!(method.sig.output, syn::ReturnType::Type(..))
    {
        return None;
    }

    let [syn::Stmt::Expr(e, _)] = method.block.stmts.as_slice() else {
        return None;
    };

    let field_ident = extract_field_from_getter_expr(e)?;
    let field_name = field_ident.to_string();

    let is_name_match = method.name == field_name
        || method.name == format!("get_{field_name}")
        || method.name.strip_prefix("is_") == Some(field_name.as_str())
        || method.name.strip_prefix("has_") == Some(field_name.as_str());

    if is_name_match {
        Some(field_name)
    } else {
        None
    }
}

/// Detects whether `method` is a trivial setter and returns the target field name.
fn detect_trivial_setter(method: &MethodInfo<'_>) -> Option<String> {
    if method.sig.inputs.len() != 2
        || !is_receiver_ref(method.sig, true)
        || !matches!(method.sig.output, syn::ReturnType::Default)
    {
        return None;
    }

    let Some(syn::FnArg::Typed(pat_type)) = method.sig.inputs.get(1) else {
        return None;
    };
    let syn::Pat::Ident(pat_ident) = &*pat_type.pat else {
        return None;
    };
    let param_name = pat_ident.ident.to_string();

    let [stmt] = method.block.stmts.as_slice() else {
        return None;
    };

    let (field_ident, assigned_param) = extract_field_and_param_from_setter_stmt(stmt)?;
    if assigned_param != &param_name {
        return None;
    }

    let field_name = field_ident.to_string();
    if method.name == format!("set_{field_name}") || method.name == field_name {
        Some(field_name)
    } else {
        None
    }
}

/// Detects whether `method` is a trivial mutable getter and returns the target field name.
fn detect_trivial_mut_getter(method: &MethodInfo<'_>) -> Option<String> {
    if method.sig.inputs.len() != 1 || !is_receiver_ref(method.sig, true) {
        return None;
    }

    let syn::ReturnType::Type(_, ty) = &method.sig.output else {
        return None;
    };
    let syn::Type::Reference(tr) = &**ty else {
        return None;
    };
    tr.mutability?;

    let [syn::Stmt::Expr(e, _)] = method.block.stmts.as_slice() else {
        return None;
    };

    let field_ident = extract_field_from_mut_getter_expr(e)?;
    let field_name = field_ident.to_string();

    let is_name_match = method.name == format!("{field_name}_mut")
        || method.name == format!("get_{field_name}_mut")
        || method.name == field_name;

    if is_name_match {
        Some(field_name)
    } else {
        None
    }
}

fn strip_parens(mut expr: &syn::Expr) -> &syn::Expr {
    while let syn::Expr::Paren(p) = expr {
        expr = &p.expr;
    }
    expr
}

fn is_self_expr(expr: &syn::Expr) -> bool {
    if let syn::Expr::Path(p) = expr {
        p.path.is_ident("self")
    } else {
        false
    }
}

fn is_full_range(expr: &syn::Expr) -> bool {
    if let syn::Expr::Range(r) = expr {
        r.start.is_none() && r.end.is_none()
    } else {
        false
    }
}

fn extract_field_from_getter_expr(expr: &syn::Expr) -> Option<&syn::Ident> {
    let expr = strip_parens(expr);
    if let syn::Expr::Return(ret) = expr {
        return ret.expr.as_deref().and_then(extract_field_from_getter_expr);
    }
    if let Some(id) = is_self_field(expr) {
        return Some(id);
    }
    if let syn::Expr::Reference(r) = expr
        && r.mutability.is_none()
    {
        let inner = strip_parens(&r.expr);
        if let Some(id) = is_self_field(inner) {
            return Some(id);
        }
        if let syn::Expr::Index(idx) = inner
            && is_full_range(strip_parens(&idx.index))
        {
            return is_self_field(strip_parens(&idx.expr));
        }
    }
    if let syn::Expr::MethodCall(call) = expr
        && call.args.is_empty()
        && matches!(
            call.method.to_string().as_str(),
            "clone" | "as_ref" | "as_str" | "as_deref"
        )
    {
        return is_self_field(strip_parens(&call.receiver));
    }
    None
}

fn extract_field_from_mut_getter_expr(expr: &syn::Expr) -> Option<&syn::Ident> {
    let expr = strip_parens(expr);
    if let syn::Expr::Return(ret) = expr {
        return ret
            .expr
            .as_deref()
            .and_then(extract_field_from_mut_getter_expr);
    }
    if let syn::Expr::Reference(r) = expr
        && r.mutability.is_some()
    {
        return is_self_field(strip_parens(&r.expr));
    }
    None
}

fn extract_field_and_param_from_setter_stmt(
    stmt: &syn::Stmt,
) -> Option<(&syn::Ident, &syn::Ident)> {
    let syn::Stmt::Expr(e, _) = stmt else {
        return None;
    };
    if let syn::Expr::Assign(assign) = strip_parens(e)
        && let Some(field_ident) = is_self_field(strip_parens(&assign.left))
        && let syn::Expr::Path(p) = strip_parens(&assign.right)
        && let Some(param_ident) = p.path.get_ident()
    {
        return Some((field_ident, param_ident));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn getter_and_setter_for_private_field_are_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Point {
    x: i32,
}

impl Point {
    pub fn x(&self) -> i32 {
        self.x
    }

    pub fn set_x(&mut self, x: i32) {
        self.x = x;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/point.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(2));
        let d0 = diags.first().ok_or("expected first diagnostic")?;
        let d1 = diags.get(1).ok_or("expected second diagnostic")?;
        assert_that!(&d0.rule, eq("purist::no_trivial_getters_setters"));
        assert_that!(
            &d0.message,
            contains_substring("Method 'x' is a trivial getter")
        );
        assert_that!(
            &d1.message,
            contains_substring("Method 'set_x' is a trivial setter")
        );
        Ok(())
    }

    #[googletest::test]
    fn public_field_with_getter_setter_combo_is_flagged() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = r#"
pub struct Point {
    pub x: i32,
}

impl Point {
    pub fn x(&self) -> i32 {
        self.x
    }
    pub fn set_x(&mut self, x: i32) {
        self.x = x;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/point.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(2));
        let d0 = diags.first().ok_or("expected first diagnostic")?;
        let d1 = diags.get(1).ok_or("expected second diagnostic")?;
        assert_that!(
            &d0.message,
            contains_substring("Method 'x' is a trivial getter")
        );
        assert_that!(
            &d1.message,
            contains_substring("Method 'set_x' is a trivial setter")
        );
        Ok(())
    }

    #[googletest::test]
    fn get_prefixed_getter_and_setter_are_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Config {
    timeout: u64,
}

impl Config {
    pub fn get_timeout(&self) -> u64 {
        self.timeout
    }

    pub fn set_timeout(&mut self, timeout: u64) {
        self.timeout = timeout;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/config.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(2));
        let d0 = diags.first().ok_or("expected first diagnostic")?;
        let d1 = diags.get(1).ok_or("expected second diagnostic")?;
        assert_that!(
            &d0.message,
            contains_substring("Method 'get_timeout' is a trivial getter")
        );
        assert_that!(
            &d1.message,
            contains_substring("Method 'set_timeout' is a trivial setter")
        );
        Ok(())
    }

    #[googletest::test]
    fn getter_and_mut_getter_are_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Item {
    count: usize,
}

impl Item {
    pub fn count(&self) -> &usize {
        &self.count
    }

    pub fn count_mut(&mut self) -> &mut usize {
        &mut self.count
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/item.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(2));
        let d0 = diags.first().ok_or("expected first diagnostic")?;
        let d1 = diags.get(1).ok_or("expected second diagnostic")?;
        assert_that!(
            &d0.message,
            contains_substring("Method 'count' is a trivial getter")
        );
        assert_that!(
            &d1.message,
            contains_substring("Method 'count_mut' is a trivial mutable getter")
        );
        Ok(())
    }

    #[googletest::test]
    fn setter_alone_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct State {
    status: u32,
}

impl State {
    pub fn set_status(&mut self, status: u32) {
        self.status = status;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/state.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn mut_getter_alone_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Counter {
    val: i64,
}

impl Counter {
    pub fn val_mut(&mut self) -> &mut i64 {
        &mut self.val
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/counter.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn read_only_getter_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct User {
    id: u64,
}

impl User {
    pub fn id(&self) -> u64 {
        self.id
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/user.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn setter_with_validation_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Age {
    value: u32,
}

impl Age {
    pub fn value(&self) -> u32 {
        self.value
    }

    pub fn set_value(&mut self, value: u32) {
        if value < 150 {
            self.value = value;
        }
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/age.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn setter_with_side_effects_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Buffer {
    size: usize,
    dirty: bool,
}

impl Buffer {
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn set_size(&mut self, size: usize) {
        self.size = size;
        self.dirty = true;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/buffer.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn builder_pattern_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Options {
    retries: u32,
}

impl Options {
    pub fn retries(&self) -> u32 {
        self.retries
    }

    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/options.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn trait_implementation_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub trait ValueHolder {
    fn value(&self) -> i32;
    fn set_value(&mut self, val: i32);
}

pub struct Holder {
    value: i32,
}

impl ValueHolder for Holder {
    fn value(&self) -> i32 {
        self.value
    }
    fn set_value(&mut self, val: i32) {
        self.value = val;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/holder.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn clap_args_struct_is_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(clap::Args)]
pub struct CliArgs {
    flag: bool,
}

impl CliArgs {
    pub fn flag(&self) -> bool {
        self.flag
    }
    pub fn set_flag(&mut self, flag: bool) {
        self.flag = flag;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/cli.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn deprecated_methods_are_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Legacy {
    val: i32,
}

impl Legacy {
    #[deprecated]
    pub fn val(&self) -> i32 {
        self.val
    }
    #[deprecated]
    pub fn set_val(&mut self, val: i32) {
        self.val = val;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/legacy.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn slice_getter_and_setter_are_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Data {
    bytes: Vec<u8>,
}

impl Data {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..]
    }
    pub fn set_bytes(&mut self, bytes: Vec<u8>) {
        self.bytes = bytes;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/data.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(2));
        Ok(())
    }

    #[googletest::test]
    fn boolean_is_prefix_getter_and_setter_are_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Switch {
    active: bool,
}

impl Switch {
    pub fn is_active(&self) -> bool {
        self.active
    }
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/switch.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(2));
        Ok(())
    }

    #[googletest::test]
    fn boolean_has_prefix_getter_and_setter_are_flagged() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = r#"
pub struct Container {
    data: bool,
}

impl Container {
    pub fn has_data(&self) -> bool {
        self.data
    }
    pub fn set_data(&mut self, data: bool) {
        self.data = data;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/container.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(2));
        Ok(())
    }

    #[googletest::test]
    fn suppression_attribute_exempts_struct_or_method() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[allow(purist::no_trivial_getters_setters)]
pub struct SuppressedPoint {
    x: i32,
}

impl SuppressedPoint {
    pub fn x(&self) -> i32 {
        self.x
    }
    pub fn set_x(&mut self, x: i32) {
        self.x = x;
    }
}

pub struct PartialSuppressed {
    y: i32,
}

impl PartialSuppressed {
    #[allow(purist::no_trivial_getters_setters)]
    pub fn y(&self) -> i32 {
        self.y
    }
    pub fn set_y(&mut self, y: i32) {
        self.y = y;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/suppressed.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoTrivialGettersSettersRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
