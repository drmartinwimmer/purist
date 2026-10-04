//! Rule: `opinionated::no_env_access_outside_config`
//!
//! # What This Rule Does
//! Flags direct invocations of `std::env::var`, `var_os`, `set_var`, or `remove_var` outside of
//! dedicated configuration, CLI, or settings modules (such as files named `config.rs`, `cli.rs`,
//! `settings.rs`, or modules located in paths matching `*config*`, `*cli*`, or `*env*`).
//!
//! # Why This Rule Exists
//! Reading or writing process environment variables throughout business logic creates hidden,
//! untracked global state dependencies. It makes components hard to test in parallel (due to race
//! conditions when mutating process environment), obscures what configuration a module actually
//! requires, and bypasses validation. All environment parameters should be loaded and validated once
//! in a dedicated configuration layer (`config.rs`) and passed down explicitly as strongly typed structs.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! // In src/database/pool.rs:
//! pub fn connect() -> Connection {
//!     // Direct environment lookup buried in database service
//!     let url = std::env::var("DATABASE_URL").expect("missing url");
//!     Connection::open(&url)
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! // In src/config.rs:
//! pub struct DatabaseConfig {
//!     pub url: String,
//! }
//!
//! impl DatabaseConfig {
//!     pub fn from_env() -> Result<Self, ConfigError> {
//!         let url = std::env::var("DATABASE_URL").map_err(...)?;
//!         Ok(Self { url })
//!     }
//! }
//!
//! // In src/database/pool.rs:
//! pub fn connect(config: &DatabaseConfig) -> Connection {
//!     Connection::open(&config.url)
//! }
//! ```

use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
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

/// Returns true if the file path is a recognized configuration or CLI module.
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

/// Visitor inspecting function call expressions for `std::env` access while tracking test scopes.
struct EnvAccessVisitor<'a> {
    /// Lint context containing file path and coordinate mapping helpers.
    ctx: &'a LintContext<'a>,
    /// Accumulated diagnostic findings.
    diagnostics: Vec<Diagnostic>,
    /// Indicates whether traversal is currently within a test function or `#[cfg(test)]` module.
    in_test_scope: bool,
}

impl<'ast> Visit<'ast> for EnvAccessVisitor<'_> {
    /// Tracks module scope and marks test scope active if annotated with `#[cfg(test)]`.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let is_cfg_test = has_cfg_test_attr(&item_mod.attrs);
        let prev = self.in_test_scope;
        if is_cfg_test {
            self.in_test_scope = true;
        }

        visit::visit_item_mod(self, item_mod);
        self.in_test_scope = prev;
    }

    /// Tracks function scope and marks test scope active if annotated with `#[test]` or `#[...::test]`.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let is_test = has_test_attr(&item_fn.attrs);
        let prev = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }

        visit::visit_item_fn(self, item_fn);
        self.in_test_scope = prev;
    }

    /// Inspects function calls outside test scopes and flags direct `std::env` queries.
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if !self.in_test_scope
            && let Some(diag) = check_env_call(self.ctx, call)
        {
            self.diagnostics.push(diag);
        }

        visit::visit_expr_call(self, call);
    }
}

/// Checks whether an attribute list includes `#[cfg(test)]`.
fn has_cfg_test_attr(attrs: &[syn::Attribute]) -> bool {
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

/// Checks whether an attribute list includes `#[test]` or `#[...::test]`.
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

/// Inspects a function call expression and returns a diagnostic if it calls `std::env::var`, `var_os`, etc.
fn check_env_call(ctx: &LintContext<'_>, call: &syn::ExprCall) -> Option<Diagnostic> {
    let syn::Expr::Path(expr_path) = &*call.func else {
        return None;
    };

    if !is_env_access_path(&expr_path.path) {
        return None;
    }

    let func_name = expr_path
        .path
        .segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_else(|| "env".to_string());
    let span = ctx.to_span(call.span());

    Some(
        Diagnostic::new(
            "opinionated::no_env_access_outside_config",
            Severity::Warning,
            format!(
                "Direct invocation of 'std::env::{func_name}' outside configuration/CLI modules. Parse environment parameters in a dedicated configuration layer."
            ),
        )
        .with_span(span)
        .with_suggested_fix("Parse environment variables in a centralized 'config.rs' or 'cli.rs' module and pass explicit parameters."),
    )
}

/// Returns true if the path targets `std::env::var`, `std::env::var_os`, `set_var`, or `remove_var`.
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
