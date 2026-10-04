use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule detecting redundant serialization roundtrips (e.g. to_string followed by from_str).
pub struct NoRedundantConversionsRule;

impl Rule for NoRedundantConversionsRule {
    fn name(&self) -> &'static str {
        "opinionated::no_redundant_conversions"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut visitor = RedundantConversionsVisitor {
            ctx,
            diagnostics: Vec::new(),
            block_vars_stack: Vec::new(),
            in_test_scope: false,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

struct RedundantConversionsVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    block_vars_stack: Vec<HashMap<String, proc_macro2::Span>>,
    in_test_scope: bool,
}

impl<'ast> Visit<'ast> for RedundantConversionsVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let is_test = item_mod.attrs.iter().any(|attr| {
            if !attr.path().is_ident("cfg") {
                return false;
            }
            let mut test_attr = false;
            drop(attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("test") {
                    test_attr = true;
                }
                Ok(())
            }));
            test_attr
        });

        let previous_test_scope = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }

        visit::visit_item_mod(self, item_mod);
        self.in_test_scope = previous_test_scope;
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let is_test = item_fn.attrs.iter().any(|attr| {
            attr.path().is_ident("test")
                || attr
                    .path()
                    .segments
                    .last()
                    .map(|s| s.ident == "test")
                    .unwrap_or(false)
        });

        let previous_test_scope = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }

        visit::visit_item_fn(self, item_fn);
        self.in_test_scope = previous_test_scope;
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        if self.in_test_scope {
            return;
        }
        let mut serializer_vars: HashMap<String, proc_macro2::Span> = HashMap::new();

        for stmt in &block.stmts {
            if let syn::Stmt::Local(local) = stmt
                && let Some(init) = &local.init
                && let Some(span) = find_nested_serializer_call(&init.expr)
                && let syn::Pat::Ident(pat_ident) = &local.pat
            {
                serializer_vars.insert(pat_ident.ident.to_string(), span);
            }
        }

        self.block_vars_stack.push(serializer_vars);
        visit::visit_block(self, block);
        self.block_vars_stack.pop();
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if self.in_test_scope {
            return;
        }

        if is_serde_parse_func(&call.func) {
            // Check nested serializer invocation
            for arg in &call.args {
                if let Some(inner_call_span) = find_nested_serializer_call(arg) {
                    let span = self.ctx.to_span(inner_call_span);
                    self.diagnostics.push(
                        Diagnostic::new(
                            "opinionated::no_redundant_conversions",
                            Severity::Warning,
                            "Redundant serialization roundtrip: value serialized and immediately deserialized.",
                        )
                        .with_span(span)
                        .with_suggested_fix("Use 'Clone::clone', 'From::from', or 'serde_json::to_value' instead of stringifying and re-parsing."),
                    );
                }
            }

            // Check sequential variable usage across active blocks
            for vars in self.block_vars_stack.iter().rev() {
                for arg in &call.args {
                    if let Some(ident) = extract_ident_from_arg(arg)
                        && let Some(&init_span) = vars.get(&ident)
                    {
                        let span = self.ctx.to_span(init_span);
                        self.diagnostics.push(
                            Diagnostic::new(
                                "opinionated::no_redundant_conversions",
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
        }

        visit::visit_expr_call(self, call);
    }
}

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

fn is_serde_parse_func(func: &syn::Expr) -> bool {
    if let syn::Expr::Path(p) = func {
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
    } else {
        false
    }
}

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
        assert_that!(&diag.rule, eq("opinionated::no_redundant_conversions"));
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
        assert_that!(&diag.rule, eq("opinionated::no_redundant_conversions"));
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
