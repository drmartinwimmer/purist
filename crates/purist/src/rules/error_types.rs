//! Rule: `purist::error_types`
//!
//! # What This Rule Does
//! Flags production functions that return unstructured string error types (`Result<T, String>` or
//! `Result<T, &str>`).
//!
//! # Why This Rule Exists
//! Returning unstructured string errors discards semantic error types, preventing callers from
//! programmatically inspecting, matching on, or recovering from specific failure modes. It forces
//! callers into fragile string-matching anti-patterns, destroys error cause chains (`#[source]`),
//! and introduces unnecessary heap allocations. Production code should define typed error enums
//! using `thiserror` (or return `anyhow::Result` in top-level application binaries).
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! pub fn parse_config(path: &Path) -> Result<Config, String> {
//!     Err("file not found".to_string())
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! #[derive(Debug, thiserror::Error)]
//! pub enum ConfigError {
//!     #[error("configuration file not found: {0}")]
//!     NotFound(PathBuf),
//! }
//!
//! pub fn parse_config(path: &Path) -> Result<Config, ConfigError> {
//!     Err(ConfigError::NotFound(path.to_path_buf()))
//! }
//! ```

use super::common::{TestScopeTracker, path_ends_with_ident};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::ReturnType;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging unstructured string error types (`Result<T, String>` or `Result<T, &str>`) in production functions.
pub struct ErrorTypesRule;

impl Rule for ErrorTypesRule {
    fn name(&self) -> &'static str {
        "purist::error_types"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut visitor = ErrorTypesVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScopeTracker::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects function return types for unstructured string errors while tracking test scopes.
struct ErrorTypesVisitor<'a> {
    /// Lint context containing file path and coordinate mapping helpers.
    ctx: &'a LintContext<'a>,
    /// Accumulated diagnostic findings.
    diagnostics: Vec<Diagnostic>,
    /// Tracks active test scope across modules and test functions.
    test_scope: TestScopeTracker,
}

impl<'ast> Visit<'ast> for ErrorTypesVisitor<'_> {
    /// Tracks module scope and marks test scope active if annotated with `#[cfg(test)]`.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let prev = self.test_scope.enter_mod(&item_mod.attrs);
        visit::visit_item_mod(self, item_mod);
        self.test_scope.exit_mod(prev);
    }

    /// Inspects free function return types and marks test scope active if annotated with `#[test]`.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let prev = self.test_scope.enter_fn(&item_fn.attrs);
        if !self.test_scope.is_in_test()
            && let Some(diag) = check_fn_return_type(self.ctx, &item_fn.sig)
        {
            self.diagnostics.push(diag);
        }

        visit::visit_item_fn(self, item_fn);
        self.test_scope.exit_fn(prev);
    }

    /// Inspects methods in inherent or trait implementations outside test scopes.
    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        let prev = self.test_scope.enter_fn(&impl_fn.attrs);
        if !self.test_scope.is_in_test()
            && let Some(diag) = check_fn_return_type(self.ctx, &impl_fn.sig)
        {
            self.diagnostics.push(diag);
        }
        visit::visit_impl_item_fn(self, impl_fn);
        self.test_scope.exit_fn(prev);
    }

    /// Inspects trait definition method signatures outside test scopes.
    fn visit_trait_item_fn(&mut self, trait_fn: &'ast syn::TraitItemFn) {
        let prev = self.test_scope.enter_fn(&trait_fn.attrs);
        if !self.test_scope.is_in_test()
            && let Some(diag) = check_fn_return_type(self.ctx, &trait_fn.sig)
        {
            self.diagnostics.push(diag);
        }
        visit::visit_trait_item_fn(self, trait_fn);
        self.test_scope.exit_fn(prev);
    }
}

/// Inspects a function signature's return type and returns a diagnostic if it returns `Result<T, String>` or `Result<T, &str>`.
fn check_fn_return_type(ctx: &LintContext<'_>, sig: &syn::Signature) -> Option<Diagnostic> {
    let return_type = match &sig.output {
        ReturnType::Type(_, ty) => ty.as_ref(),
        ReturnType::Default => return None,
    };

    let (error_ty, err_desc) = detect_string_error_type(return_type)?;
    let span = ctx.to_span(error_ty.span());
    let fn_name = sig.ident.to_string();

    Some(
        Diagnostic::new(
            "purist::error_types",
            Severity::Warning,
            format!(
                "Function '{fn_name}' returns unstructured error type '{err_desc}'. Use structured error enums via thiserror or anyhow::Result."
            ),
        )
        .with_span(span)
        .with_suggested_fix("Define a dedicated error enum deriving 'thiserror::Error' or use 'anyhow::Result'."),
    )
}

/// Detects if a return type is `Result<T, String>` or `Result<T, &str>`, returning the inner error type and description.
fn detect_string_error_type(ty: &syn::Type) -> Option<(&syn::Type, String)> {
    let syn::Type::Path(type_path) = ty else {
        return None;
    };

    if !path_ends_with_ident(&type_path.path, "Result") {
        return None;
    }

    let last_segment = type_path.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(ab) = &last_segment.arguments else {
        return None;
    };

    if ab.args.len() != 2 {
        return None;
    }

    let syn::GenericArgument::Type(err_arg) = ab.args.iter().nth(1)? else {
        return None;
    };

    if is_string_type(err_arg) {
        return Some((err_arg, "String".to_string()));
    }

    if is_str_ref_type(err_arg) {
        return Some((err_arg, "&str".to_string()));
    }

    None
}

/// Returns true if the type is `String`.
fn is_string_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(p) = ty {
        return path_ends_with_ident(&p.path, "String");
    }
    false
}

/// Returns true if the type is `&str`.
fn is_str_ref_type(ty: &syn::Type) -> bool {
    if let syn::Type::Reference(r) = ty
        && let syn::Type::Path(p) = &*r.elem
    {
        return path_ends_with_ident(&p.path, "str");
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn result_string_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn compute() -> Result<i32, String> { Ok(42) }\n";
        let ctx = LintContext::new(Path::new("src/compute.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ErrorTypesRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::error_types"));
        assert_that!(
            &diag.message,
            contains_substring("returns unstructured error type 'String'")
        );
        Ok(())
    }

    #[googletest::test]
    fn result_str_ref_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn lookup() -> Result<usize, &'static str> { Ok(0) }\n";
        let ctx = LintContext::new(Path::new("src/lookup.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ErrorTypesRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::error_types"));
        assert_that!(
            &diag.message,
            contains_substring("returns unstructured error type '&str'")
        );
        Ok(())
    }

    #[googletest::test]
    fn result_custom_error_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn execute() -> Result<(), MyError> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/exec.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ErrorTypesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn result_string_in_test_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "#[test]\nfn parse_action() -> Result<(), String> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ErrorTypesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
