//! Rule: `purist::no_redundant_conversions`
//!
//! # What This Rule Does
//! Detects redundant serialization roundtrips where a data structure is serialized (e.g. via
//! `serde_json::to_string` or `to_vec`) and then immediately deserialized back (via `from_str` or `from_slice`),
//! either nested in a single expression or sequentially via local variables. Test scopes are exempt.
//!
//! # Why This Rule Exists
//! Developers sometimes serialize and deserialize objects as a quick way to deep-clone, convert
//! between similar structs, or detach lifetimes. This pattern is extremely inefficient: it converts
//! structured memory into JSON text and re-parses it, allocating intermediate strings and running
//! full tokenization/parsing logic. Code should derive `Clone`, implement `From`/`Into`, or use
//! direct struct transformation functions.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! pub fn clone_payload(payload: &Payload) -> Payload {
//!     // Inefficient serialization roundtrip used as a deep clone
//!     serde_json::from_str(&serde_json::to_string(payload).unwrap()).unwrap()
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! pub fn clone_payload(payload: &Payload) -> Payload {
//!     payload.clone() // Idiomatic, efficient in-memory clone
//! }
//! ```

use super::common::TestScopeTracker;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule detecting redundant serialization roundtrips (e.g. `to_string` followed by `from_str`).
pub struct NoRedundantConversionsRule;

impl Rule for NoRedundantConversionsRule {
    fn name(&self) -> &'static str {
        "purist::no_redundant_conversions"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut visitor = RedundantConversionsVisitor {
            ctx,
            diagnostics: Vec::new(),
            block_vars_stack: Vec::new(),
            test_scope: TestScopeTracker::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor tracking local variable assignments and call expressions to detect serialization roundtrips.
struct RedundantConversionsVisitor<'a> {
    /// Lint context containing file path and coordinate mapping helpers.
    ctx: &'a LintContext<'a>,
    /// Accumulated diagnostic findings.
    diagnostics: Vec<Diagnostic>,
    /// Stack of lexical block scopes tracking variables assigned from serializer outputs.
    block_vars_stack: Vec<HashMap<String, proc_macro2::Span>>,
    /// Tracks active test scope across modules and test functions.
    test_scope: TestScopeTracker,
}

impl<'ast> Visit<'ast> for RedundantConversionsVisitor<'_> {
    /// Tracks module scope and marks test scope active if annotated with `#[cfg(test)]`.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.test_scope.push_mod(&item_mod.attrs);
        visit::visit_item_mod(self, item_mod);
        self.test_scope.pop();
    }

    /// Tracks function scope and marks test scope active if annotated with `#[test]` or `#[...::test]`.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        self.test_scope.push_fn(&item_fn.attrs);
        visit::visit_item_fn(self, item_fn);
        self.test_scope.pop();
    }

    /// Maintains the lexical block scope stack, recording variables initialized from serializer calls.
    fn visit_block(&mut self, block: &'ast syn::Block) {
        if self.test_scope.is_in_test() {
            return;
        }

        let serializer_vars = collect_block_serializer_vars(block);
        self.block_vars_stack.push(serializer_vars);

        visit::visit_block(self, block);

        self.block_vars_stack.pop();
    }

    /// Inspects call expressions and flags nested or sequential deserialization of serialized variables.
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if !self.test_scope.is_in_test() {
            self.check_call_redundant_conversion(call);
        }

        visit::visit_expr_call(self, call);
    }
}

impl RedundantConversionsVisitor<'_> {
    fn check_call_redundant_conversion(&mut self, call: &syn::ExprCall) {
        let findings = check_redundant_conversion_call(self.ctx, call, &self.block_vars_stack);
        self.diagnostics.extend(findings);
    }
}

/// Scans local variable declarations in a block and records variables initialized with serializer calls.
fn collect_block_serializer_vars(block: &syn::Block) -> HashMap<String, proc_macro2::Span> {
    let mut serializer_vars = HashMap::new();
    for stmt in &block.stmts {
        if let syn::Stmt::Local(local) = stmt
            && let Some(init) = &local.init
            && let Some(span) = find_nested_serializer_call(&init.expr)
            && let syn::Pat::Ident(pat_ident) = &local.pat
        {
            serializer_vars.insert(pat_ident.ident.to_string(), span);
        }
    }
    serializer_vars
}

/// Checks whether a call expression is a deserializer operating on an immediately serialized value or variable.
fn check_redundant_conversion_call(
    ctx: &LintContext<'_>,
    call: &syn::ExprCall,
    block_vars_stack: &[HashMap<String, proc_macro2::Span>],
) -> Vec<Diagnostic> {
    let mut findings = Vec::new();

    if !is_serde_parse_func(&call.func) {
        return findings;
    }

    // 1. Check nested serializer invocation (e.g. from_str(&to_string(data)...))
    for arg in &call.args {
        if let Some(inner_call_span) = find_nested_serializer_call(arg) {
            let span = ctx.to_span(inner_call_span);
            findings.push(
                Diagnostic::new(
                    "purist::no_redundant_conversions",
                    Severity::Warning,
                    "Redundant serialization roundtrip: value serialized and immediately deserialized.",
                )
                .with_span(span)
                .with_suggested_fix("Use 'Clone::clone', 'From::from', or 'serde_json::to_value' instead of stringifying and re-parsing."),
            );
        }
    }

    // 2. Check sequential variable usage across active lexical blocks
    for vars in block_vars_stack.iter().rev() {
        for arg in &call.args {
            if let Some(ident) = extract_ident_from_arg(arg)
                && let Some(&init_span) = vars.get(&ident)
            {
                let span = ctx.to_span(init_span);
                findings.push(
                    Diagnostic::new(
                        "purist::no_redundant_conversions",
                        Severity::Warning,
                        format!(
                            "Redundant serialization roundtrip: variable '{ident}' serialized and immediately deserialized."
                        ),
                    )
                    .with_span(span)
                    .with_suggested_fix("Use 'Clone::clone', 'From::from', or direct mapping instead of roundtrip serialization."),
                );
            }
        }
    }

    findings
}

/// Extracts a variable identifier from an argument expression, unwrapping references, try operators, and method calls.
fn extract_ident_from_arg(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(p) => p.path.get_ident().map(|i| i.to_string()),
        syn::Expr::Reference(r) => extract_ident_from_arg(&r.expr),
        syn::Expr::Try(t) => extract_ident_from_arg(&t.expr),
        syn::Expr::Paren(p) => extract_ident_from_arg(&p.expr),
        syn::Expr::MethodCall(mc) => {
            let m = mc.method.to_string();
            if m == "as_str" || m == "as_bytes" || m == "clone" {
                extract_ident_from_arg(&mc.receiver)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Returns true if a function expression targets `serde_json::from_str` or `from_slice`.
fn is_serde_parse_func(func: &syn::Expr) -> bool {
    let syn::Expr::Path(p) = func else {
        return false;
    };

    let path_str = p
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::");

    path_str == "serde_json::from_str"
        || path_str == "serde_json::from_slice"
        || path_str == "from_str"
        || path_str == "from_slice"
}

/// Recursively detects whether an expression invokes a serialization function like `to_string` or `to_vec`.
fn find_nested_serializer_call(expr: &syn::Expr) -> Option<proc_macro2::Span> {
    match expr {
        syn::Expr::Call(call) => {
            if let syn::Expr::Path(p) = &*call.func {
                let path_str = p
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::");
                if path_str == "serde_json::to_string"
                    || path_str == "serde_json::to_vec"
                    || path_str == "to_string"
                    || path_str == "to_vec"
                {
                    return Some(call.span());
                }
            }
            None
        }
        syn::Expr::MethodCall(mc) => {
            let m = mc.method.to_string();
            if m == "unwrap" || m == "expect" || m == "as_str" || m == "as_bytes" {
                find_nested_serializer_call(&mc.receiver)
            } else {
                None
            }
        }
        syn::Expr::Reference(r) => find_nested_serializer_call(&r.expr),
        syn::Expr::Try(t) => find_nested_serializer_call(&t.expr),
        syn::Expr::Paren(p) => find_nested_serializer_call(&p.expr),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn nested_redundant_conversion_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn duplicate(data: &MyData) -> MyData {
    serde_json::from_str(&serde_json::to_string(data).unwrap()).unwrap()
}
"#;
        let ctx = LintContext::new(Path::new("src/dup.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoRedundantConversionsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_redundant_conversions"));
        Ok(())
    }

    #[googletest::test]
    fn sequential_redundant_conversion_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn convert(data: &MyData) -> MyData {
    let serialized = serde_json::to_string(data).unwrap();
    let cloned: MyData = serde_json::from_str(&serialized).unwrap();
    cloned
}
"#;
        let ctx = LintContext::new(Path::new("src/conv.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoRedundantConversionsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_redundant_conversions"));
        assert_that!(&diag.message, contains_substring("variable 'serialized'"));
        Ok(())
    }

    #[googletest::test]
    fn unrelated_json_operations_are_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn parse_incoming(raw: &str) -> MyData {
    serde_json::from_str(raw).unwrap()
}
"#;
        let ctx = LintContext::new(Path::new("src/parse.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoRedundantConversionsRule.check_file(&ctx, &ast);
        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn roundtrip_in_test_fn_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn test_roundtrip() {
    let serialized = serde_json::to_string(&data).unwrap();
    let deserialized: MyData = serde_json::from_str(&serialized).unwrap();
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoRedundantConversionsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn roundtrip_in_cfg_test_mod_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[cfg(test)]
mod tests {
    fn helper() {
        let serialized = serde_json::to_string(&data).unwrap();
        let deserialized: MyData = serde_json::from_str(&serialized).unwrap();
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoRedundantConversionsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
