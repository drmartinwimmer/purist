//! # Rule: `purist::no_bifurcated_trait_dispatch`
//!
//! ## What This Rule Does
//! Flags conditional branches (`if`/`else` or `match`) that instantiate different local sibling types
//! implementing the same trait (or defining the same inherent method) to dispatch the exact same
//! method on the exact same target input.
//!
//! ## Why This Rule Exists
//! Branching on an argument or condition to instantiate and run alternate sibling types that implement
//! the same trait is a classic anti-pattern: *bifurcated trait dispatch* / *flag-driven algorithm selection*.
//!
//! This pattern indicates that instead of maintaining separate types with parallel implementations
//! and switching between them in an ad-hoc conditional branch:
//! 1. The logic should be unified into a single parameterized type (e.g. `Checker::new(mode)`),
//! 2. Dynamic dispatch / trait objects should be used (`let checker: Box<dyn Trait> = ...; checker.execute(...)`), or
//! 3. Type selection should be encapsulated in a factory or constructor rather than bifurcating the calling algorithm.
//!
//! It frequently leads to code duplication, subtle behavioral drift between branches, and copy-paste type explosion.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! if in_visitor {
//!     let mut checker = ChildCollectionLoopChecker::new();
//!     checker.visit_block(block);
//! } else {
//!     let mut checker = NonVisitorAstLoopChecker::new();
//!     checker.visit_block(block);
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! // Unified parameterized type
//! let mut checker = AstChildLoopChecker::new(in_visitor);
//! checker.visit_block(block);
//! ```

use std::collections::{HashMap, HashSet};

use super::common::{
    TestScope, WithTestScope, extract_type_ident, has_suppression_attribute, path_last_ident,
};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule forbidding conditional branches from instantiating alternate sibling types to dispatch the same trait method.
pub struct NoBifurcatedTraitDispatchRule;

impl Rule for NoBifurcatedTraitDispatchRule {
    fn name(&self) -> &'static str {
        "purist::no_bifurcated_trait_dispatch"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let registry = FileTypeRegistry::collect(file);
        let mut visitor = DispatchVisitor {
            ctx,
            registry: &registry,
            diagnostics: Vec::new(),
            test_scope: TestScope::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Registry of types, implemented traits, and inherent methods within the analyzed file.
struct FileTypeRegistry {
    local_types: HashSet<String>,
    type_to_traits: HashMap<String, HashSet<String>>,
    type_to_inherent_methods: HashMap<String, HashSet<String>>,
}

impl FileTypeRegistry {
    fn collect(file: &syn::File) -> Self {
        let mut local_types = HashSet::new();
        let mut type_to_traits: HashMap<String, HashSet<String>> = HashMap::new();
        let mut type_to_inherent_methods: HashMap<String, HashSet<String>> = HashMap::new();

        for item in &file.items {
            match item {
                syn::Item::Struct(s) => {
                    local_types.insert(s.ident.to_string());
                }
                syn::Item::Enum(e) => {
                    local_types.insert(e.ident.to_string());
                }
                syn::Item::Impl(item_impl) => {
                    process_item_impl(
                        item_impl,
                        &mut type_to_traits,
                        &mut type_to_inherent_methods,
                    );
                }
                _ => {}
            }
        }

        Self {
            local_types,
            type_to_traits,
            type_to_inherent_methods,
        }
    }

    /// Determines if two types are sibling types in this file sharing a trait or inherent method.
    /// Returns `Some(Some(trait_name))` if a shared trait is found, `Some(None)` if sibling local types,
    /// or `None` if they are not related sibling types.
    fn find_sibling_relationship(
        &self,
        type_a: &str,
        type_b: &str,
        method_name: &str,
    ) -> Option<Option<String>> {
        if type_a == type_b {
            return None;
        }

        let a_is_local =
            self.local_types.contains(type_a) || self.type_to_traits.contains_key(type_a);
        let b_is_local =
            self.local_types.contains(type_b) || self.type_to_traits.contains_key(type_b);
        if !a_is_local || !b_is_local {
            return None;
        }

        if let (Some(traits_a), Some(traits_b)) = (
            self.type_to_traits.get(type_a),
            self.type_to_traits.get(type_b),
        ) && let Some(shared_trait) = traits_a.intersection(traits_b).next()
        {
            return Some(Some(shared_trait.clone()));
        }

        if let (Some(methods_a), Some(methods_b)) = (
            self.type_to_inherent_methods.get(type_a),
            self.type_to_inherent_methods.get(type_b),
        ) && methods_a.contains(method_name)
            && methods_b.contains(method_name)
        {
            return Some(None);
        }

        if self.local_types.contains(type_a) && self.local_types.contains(type_b) {
            return Some(None);
        }

        None
    }
}

/// Helper to index implemented traits and inherent methods from an `impl` block.
fn process_item_impl(
    item_impl: &syn::ItemImpl,
    type_to_traits: &mut HashMap<String, HashSet<String>>,
    type_to_inherent_methods: &mut HashMap<String, HashSet<String>>,
) {
    let Some(self_ident) = extract_type_ident(&item_impl.self_ty) else {
        return;
    };
    let type_name = self_ident.to_string();

    if let Some((trait_path, _)) = &item_impl.trait_ {
        if let Some(trait_ident) = path_last_ident(trait_path) {
            type_to_traits
                .entry(type_name)
                .or_default()
                .insert(trait_ident.to_string());
        }
        return;
    }

    for impl_item in &item_impl.items {
        if let syn::ImplItem::Fn(impl_fn) = impl_item {
            type_to_inherent_methods
                .entry(type_name.clone())
                .or_default()
                .insert(impl_fn.sig.ident.to_string());
        }
    }
}

/// Represents a candidate dispatch: an instantiation of `type_name` followed by a call to `method_name`.
#[derive(Clone, Debug)]
struct DispatchCandidate {
    type_name: String,
    method_name: String,
    target_arg: Option<String>,
}

/// Visitor that inspects conditional branches for bifurcated trait dispatch.
#[derive(WithTestScope)]
struct DispatchVisitor<'a> {
    ctx: &'a LintContext<'a>,
    registry: &'a FileTypeRegistry,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScope,
}

impl<'ast> Visit<'ast> for DispatchVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        if has_suppression_attribute(&item_fn.attrs, "no_bifurcated_trait_dispatch") {
            return;
        }
        self.with_test_fn(&item_fn.attrs, |this| {
            visit::visit_item_fn(this, item_fn);
        });
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        if has_suppression_attribute(&impl_fn.attrs, "no_bifurcated_trait_dispatch") {
            return;
        }
        self.with_test_fn(&impl_fn.attrs, |this| {
            visit::visit_impl_item_fn(this, impl_fn);
        });
    }

    fn visit_trait_item_fn(&mut self, trait_fn: &'ast syn::TraitItemFn) {
        if has_suppression_attribute(&trait_fn.attrs, "no_bifurcated_trait_dispatch") {
            return;
        }
        self.with_test_fn(&trait_fn.attrs, |this| {
            visit::visit_trait_item_fn(this, trait_fn);
        });
    }

    fn visit_expr_if(&mut self, expr_if: &'ast syn::ExprIf) {
        if !self.test_scope.is_in_test() {
            self.check_expr_if_bifurcated_dispatch(expr_if);
        }
        visit::visit_expr_if(self, expr_if);
    }

    fn visit_expr_match(&mut self, expr_match: &'ast syn::ExprMatch) {
        if !self.test_scope.is_in_test() {
            self.check_expr_match_bifurcated_dispatch(expr_match);
        }
        visit::visit_expr_match(self, expr_match);
    }
}

impl DispatchVisitor<'_> {
    fn check_expr_if_bifurcated_dispatch(&mut self, expr_if: &syn::ExprIf) {
        let Some((_, else_expr)) = &expr_if.else_branch else {
            return;
        };

        let then_candidate = extract_dispatch_from_block(&expr_if.then_branch);
        let else_candidate = extract_dispatch_from_expr(else_expr);

        if let (Some(d1), Some(d2)) = (then_candidate, else_candidate) {
            self.evaluate_pair(&d1, &d2, expr_if.span());
        }
    }

    fn check_expr_match_bifurcated_dispatch(&mut self, expr_match: &syn::ExprMatch) {
        let mut collector = MatchArmDispatchCollector::default();
        collector.visit_expr_match(expr_match);

        for i in 0..collector.candidates.len() {
            for j in (i + 1)..collector.candidates.len() {
                if let (Some(c1), Some(c2)) =
                    (collector.candidates.get(i), collector.candidates.get(j))
                    && self.evaluate_pair(c1, c2, expr_match.span())
                {
                    return;
                }
            }
        }
    }

    fn evaluate_pair(
        &mut self,
        d1: &DispatchCandidate,
        d2: &DispatchCandidate,
        span: proc_macro2::Span,
    ) -> bool {
        if d1.type_name == d2.type_name || d1.method_name != d2.method_name {
            return false;
        }

        let targets_match = match (&d1.target_arg, &d2.target_arg) {
            (Some(a), Some(b)) => a == b,
            (None, None) => true,
            _ => false,
        };

        if !targets_match {
            return false;
        }

        let Some(shared_trait_opt) =
            self.registry
                .find_sibling_relationship(&d1.type_name, &d2.type_name, &d1.method_name)
        else {
            return false;
        };

        let diag_span = self.ctx.to_span(span);
        let type_a = &d1.type_name;
        let type_b = &d2.type_name;
        let method_name = &d1.method_name;

        let message = if let Some(trait_name) = &shared_trait_opt {
            format!(
                "Bifurcated dispatch to sibling types '{type_a}' and '{type_b}' implementing trait '{trait_name}' on method '{method_name}'. Consolidate into a single parameterized type or unify dispatch outside the branch."
            )
        } else {
            format!(
                "Bifurcated dispatch to sibling types '{type_a}' and '{type_b}' on method '{method_name}'. Consolidate into a single parameterized type or unify dispatch outside the branch."
            )
        };

        self.diagnostics.push(
            Diagnostic::new(
                "purist::no_bifurcated_trait_dispatch",
                Severity::Warning,
                message,
            )
            .with_span(diag_span)
            .with_suggested_fix(format!(
                "Consolidate '{type_a}' and '{type_b}' into a single parameterized type, or instantiate a trait object/enum before dispatching."
            )),
        );
        true
    }
}

/// Visitor that collects dispatch candidates from match arms using the specialized `visit_arm` hook.
#[derive(Default)]
struct MatchArmDispatchCollector {
    candidates: Vec<DispatchCandidate>,
}

impl<'ast> Visit<'ast> for MatchArmDispatchCollector {
    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        if let Some(cand) = extract_dispatch_from_expr(&arm.body) {
            self.candidates.push(cand);
        }
        visit::visit_arm(self, arm);
    }
}

fn extract_dispatch_from_expr(expr: &syn::Expr) -> Option<DispatchCandidate> {
    match expr {
        syn::Expr::Block(expr_block) => extract_dispatch_from_block(&expr_block.block),
        syn::Expr::If(inner_if) => extract_dispatch_from_block(&inner_if.then_branch),
        syn::Expr::MethodCall(call) => {
            let type_name = extract_constructed_type(&call.receiver)?;
            let method_name = call.method.to_string();
            let target_arg = call.args.first().and_then(extract_target_arg_name);
            Some(DispatchCandidate {
                type_name,
                method_name,
                target_arg,
            })
        }
        _ => None,
    }
}

fn extract_dispatch_from_block(block: &syn::Block) -> Option<DispatchCandidate> {
    // Pattern 1: `let [mut] var = Type::new(...); ... var.method(target);`
    for (idx, stmt) in block.stmts.iter().enumerate() {
        if let syn::Stmt::Local(local) = stmt {
            let (var_name, type_from_pat) = extract_local_ident_and_type(local);
            let type_name = type_from_pat.or_else(|| {
                local
                    .init
                    .as_ref()
                    .and_then(|init| extract_constructed_type(&init.expr))
            });

            if let (Some(var), Some(t_name)) = (var_name, type_name)
                && let Some(remaining) = block.stmts.get(idx + 1..)
                && let Some(candidate) = find_dispatch_after_let(remaining, &var, t_name)
            {
                return Some(candidate);
            }
        }
    }

    // Pattern 2: Directly chained `Type::new(...).method(target)` statement or trailing expression
    for stmt in &block.stmts {
        if let syn::Stmt::Expr(syn::Expr::MethodCall(call), _) = stmt
            && let Some(type_name) = extract_constructed_type(&call.receiver)
        {
            let method_name = call.method.to_string();
            let target_arg = call.args.first().and_then(extract_target_arg_name);
            return Some(DispatchCandidate {
                type_name,
                method_name,
                target_arg,
            });
        }
    }

    None
}

/// Extracts variable name and optional type annotation from a `let` statement pattern.
fn extract_local_ident_and_type(local: &syn::Local) -> (Option<String>, Option<String>) {
    match &local.pat {
        syn::Pat::Ident(pat_ident) => (Some(pat_ident.ident.to_string()), None),
        syn::Pat::Type(pat_type) => {
            let var_name = if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                Some(pat_ident.ident.to_string())
            } else {
                None
            };
            let type_name = extract_type_ident(&pat_type.ty).map(|i| i.to_string());
            (var_name, type_name)
        }
        _ => (None, None),
    }
}

/// Finds the first method dispatch on `var_name` in subsequent statements.
fn find_dispatch_after_let(
    subsequent_stmts: &[syn::Stmt],
    var_name: &str,
    type_name: String,
) -> Option<DispatchCandidate> {
    for stmt in subsequent_stmts {
        if let Some((method_name, target_arg)) = find_method_call_on_var(stmt, var_name) {
            return Some(DispatchCandidate {
                type_name,
                method_name,
                target_arg,
            });
        }
    }
    None
}

/// Extracts a constructed type name from an expression (e.g. `Type::new(...)`, `Type { ... }`, `Type(...)`).
fn extract_constructed_type(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Call(call) => match &*call.func {
            syn::Expr::Path(path) => {
                let segments = &path.path.segments;
                if segments.len() >= 2 {
                    segments.iter().rev().nth(1).map(|s| s.ident.to_string())
                } else {
                    segments.first().map(|s| s.ident.to_string())
                }
            }
            _ => None,
        },
        syn::Expr::Struct(s) => path_last_ident(&s.path).map(|i| i.to_string()),
        syn::Expr::Path(p) => {
            if p.path.segments.len() >= 2 {
                p.path
                    .segments
                    .iter()
                    .rev()
                    .nth(1)
                    .map(|s| s.ident.to_string())
            } else {
                p.path.segments.first().map(|s| s.ident.to_string())
            }
        }
        _ => None,
    }
}

/// Searches an AST statement for a method invocation on `var_name`.
fn find_method_call_on_var(stmt: &syn::Stmt, var_name: &str) -> Option<(String, Option<String>)> {
    struct MethodFinder<'a> {
        target_var: &'a str,
        found: Option<(String, Option<String>)>,
    }

    impl<'ast> Visit<'ast> for MethodFinder<'_> {
        fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
            if self.found.is_some() {
                return;
            }
            if let syn::Expr::Path(path) = &*call.receiver
                && path.path.is_ident(self.target_var)
            {
                let method = call.method.to_string();
                let target = call.args.first().and_then(extract_target_arg_name);
                self.found = Some((method, target));
                return;
            }
            visit::visit_expr_method_call(self, call);
        }
    }

    let mut finder = MethodFinder {
        target_var: var_name,
        found: None,
    };
    finder.visit_stmt(stmt);
    finder.found
}

/// Extracts a normalized representation of an argument target expression.
fn extract_target_arg_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(p) => path_last_ident(&p.path).map(|i| i.to_string()),
        syn::Expr::Reference(r) => extract_target_arg_name(&r.expr),
        syn::Expr::Unary(u) if matches!(u.op, syn::UnOp::Deref(_)) => {
            extract_target_arg_name(&u.expr)
        }
        syn::Expr::Field(f) => {
            if let syn::Member::Named(ident) = &f.member {
                if let Some(base) = extract_target_arg_name(&f.base) {
                    Some(format!("{base}.{ident}"))
                } else {
                    Some(ident.to_string())
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use googletest::prelude::*;

    use super::*;

    #[googletest::test]
    fn bifurcated_dispatch_in_if_else_with_shared_trait_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use syn::visit::Visit;

struct VisitorA;
impl<'ast> Visit<'ast> for VisitorA {}

struct VisitorB;
impl<'ast> Visit<'ast> for VisitorB {}

fn inspect(flag: bool, block: &syn::Block) {
    if flag {
        let mut a = VisitorA;
        a.visit_block(block);
    } else {
        let mut b = VisitorB;
        b.visit_block(block);
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBifurcatedTraitDispatchRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            &diag.message,
            contains_substring(
                "Bifurcated dispatch to sibling types 'VisitorA' and 'VisitorB' implementing trait 'Visit' on method 'visit_block'"
            )
        );
        Ok(())
    }

    #[googletest::test]
    fn bifurcated_dispatch_in_match_with_shared_trait_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
trait Handler {
    fn handle(&self, req: &str);
}

struct HandlerA;
impl Handler for HandlerA {
    fn handle(&self, _req: &str) {}
}

struct HandlerB;
impl Handler for HandlerB {
    fn handle(&self, _req: &str) {}
}

enum Mode { A, B }

fn dispatch(mode: Mode, req: &str) {
    match mode {
        Mode::A => {
            let h = HandlerA;
            h.handle(req);
        }
        Mode::B => {
            let h = HandlerB;
            h.handle(req);
        }
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBifurcatedTraitDispatchRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            &diag.message,
            contains_substring(
                "Bifurcated dispatch to sibling types 'HandlerA' and 'HandlerB' implementing trait 'Handler' on method 'handle'"
            )
        );
        Ok(())
    }

    #[googletest::test]
    fn chained_bifurcated_dispatch_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
trait Processor {
    fn process(&self, input: &str);
}

struct FastProcessor;
impl Processor for FastProcessor {
    fn process(&self, _input: &str) {}
}

struct SlowProcessor;
impl Processor for SlowProcessor {
    fn process(&self, _input: &str) {}
}

fn run(is_fast: bool, input: &str) {
    if is_fast {
        FastProcessor.process(input);
    } else {
        SlowProcessor.process(input);
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBifurcatedTraitDispatchRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            &diag.message,
            contains_substring(
                "Bifurcated dispatch to sibling types 'FastProcessor' and 'SlowProcessor' implementing trait 'Processor' on method 'process'"
            )
        );
        Ok(())
    }

    #[googletest::test]
    fn single_parameterized_type_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use syn::visit::Visit;

struct UnifiedVisitor {
    flag: bool,
}
impl<'ast> Visit<'ast> for UnifiedVisitor {}

fn inspect(flag: bool, block: &syn::Block) {
    let mut visitor = UnifiedVisitor { flag };
    visitor.visit_block(block);
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBifurcatedTraitDispatchRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn same_type_instantiation_in_branches_is_permitted() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = r#"
struct Worker {
    mode: u32,
}

impl Worker {
    fn run(&self, data: &str) {}
}

fn execute(flag: bool, data: &str) {
    if flag {
        let w = Worker { mode: 1 };
        w.run(data);
    } else {
        let w = Worker { mode: 2 };
        w.run(data);
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBifurcatedTraitDispatchRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn different_target_arguments_are_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
trait Worker {
    fn run(&self, data: &str);
}

struct WorkerA;
impl Worker for WorkerA { fn run(&self, _data: &str) {} }

struct WorkerB;
impl Worker for WorkerB { fn run(&self, _data: &str) {} }

fn execute(flag: bool, data_a: &str, data_b: &str) {
    if flag {
        let w = WorkerA;
        w.run(data_a);
    } else {
        let w = WorkerB;
        w.run(data_b);
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBifurcatedTraitDispatchRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn cfg_test_code_is_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[cfg(test)]
mod tests {
    use syn::visit::Visit;

    struct VisitorA;
    impl<'ast> Visit<'ast> for VisitorA {}

    struct VisitorB;
    impl<'ast> Visit<'ast> for VisitorB {}

    #[test]
    fn test_dispatch(flag: bool, block: &syn::Block) {
        if flag {
            let mut a = VisitorA;
            a.visit_block(block);
        } else {
            let mut b = VisitorB;
            b.visit_block(block);
        }
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBifurcatedTraitDispatchRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn suppression_attribute_exempts_function() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use syn::visit::Visit;

struct VisitorA;
impl<'ast> Visit<'ast> for VisitorA {}

struct VisitorB;
impl<'ast> Visit<'ast> for VisitorB {}

#[allow(purist::no_bifurcated_trait_dispatch)]
fn inspect(flag: bool, block: &syn::Block) {
    if flag {
        let mut a = VisitorA;
        a.visit_block(block);
    } else {
        let mut b = VisitorB;
        b.visit_block(block);
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBifurcatedTraitDispatchRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
