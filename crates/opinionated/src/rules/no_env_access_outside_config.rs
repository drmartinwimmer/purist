use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging raw `std::env::var` / `set_var` calls outside dedicated config or CLI modules.
pub struct NoEnvAccessOutsideConfigRule;

impl Rule for NoEnvAccessOutsideConfigRule {
    fn name(&self) -> &'static str {
        "opinionated::no_env_access_outside_config"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if is_exempt_config_path(ctx) {
            return Vec::new();
        }

        let mut visitor = EnvAccessVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_test_scope: false,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

fn is_exempt_config_path(ctx: &LintContext<'_>) -> bool {
    if ctx.is_test_file() {
        return true;
    }

    let path_str = ctx.file_path().to_string_lossy().to_ascii_lowercase();
    path_str.contains("config")
        || path_str.contains("cli")
        || path_str.contains("settings")
        || path_str.contains("env")
}

struct EnvAccessVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_test_scope: bool,
}

impl<'ast> Visit<'ast> for EnvAccessVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let is_cfg_test = item_mod.attrs.iter().any(|attr| {
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
        });

        let prev = self.in_test_scope;
        if is_cfg_test {
            self.in_test_scope = true;
        }

        visit::visit_item_mod(self, item_mod);
        self.in_test_scope = prev;
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

        let prev = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }

        visit::visit_item_fn(self, item_fn);
        self.in_test_scope = prev;
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if !self.in_test_scope
            && let syn::Expr::Path(expr_path) = &*call.func
            && is_env_access_path(&expr_path.path)
        {
            let func_name = expr_path
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_else(|| "env".to_string());
            let span = self.ctx.to_span(call.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "opinionated::no_env_access_outside_config",
                    Severity::Warning,
                    format!(
                        "Direct invocation of 'std::env::{func_name}' outside configuration/CLI modules. Parse environment parameters in a dedicated configuration layer."
                    ),
                )
                .with_span(span)
                .with_suggested_fix("Parse environment variables in a centralized 'config.rs' or 'cli.rs' module and pass explicit parameters."),
            );
        }

        visit::visit_expr_call(self, call);
    }
}

fn is_env_access_path(path: &syn::Path) -> bool {
    let segments: Vec<&syn::PathSegment> = path.segments.iter().collect();
    if segments.is_empty() {
        return false;
    }

    let last_ident = segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default();
    if !matches!(
        last_ident.as_str(),
        "var" | "var_os" | "set_var" | "remove_var"
    ) {
        return false;
    }

    if segments.len() == 1 {
        return false;
    }

    if let Some(second) = segments.get(segments.len().saturating_sub(2)) {
        return second.ident == "env";
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn env_var_in_domain_service_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn connect() {
    let _ = std::env::var("DATABASE_URL");
}
"#;
        let ctx = LintContext::new(Path::new("src/services/db.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoEnvAccessOutsideConfigRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::no_env_access_outside_config"));
        assert_that!(&diag.message, contains_substring("std::env::var"));
        Ok(())
    }

    #[googletest::test]
    fn env_var_in_config_file_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn load() {
    let _ = std::env::var("PORT");
}
"#;
        let ctx = LintContext::new(Path::new("src/config.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoEnvAccessOutsideConfigRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn env_var_in_test_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn reads_env_key() {
    let _ = std::env::var("TEST_KEY");
}
"#;
        let ctx = LintContext::new(Path::new("src/services/db.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoEnvAccessOutsideConfigRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
