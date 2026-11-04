//! Rule: `purist::error_types`
//!
//! # What This Rule Does
//! Enforces structured, idiomatically named error types in production code:
//! 1. Flags functions that return unstructured string error types (`Result<T, String>` or
//!    `Result<T, &str>`).
//! 2. Flags error structs or enums that are named bare `Error` or derive `thiserror::Error`
//!    without following the `<Subject>Error` naming convention.
//! 3. Flags manual implementations of `std::error::Error` (or `core::error::Error`), requiring
//!    `#[derive(thiserror::Error)]` instead.
//!
//! # Why This Rule Exists
//! Returning unstructured string errors discards semantic error types, preventing callers from
//! programmatically inspecting, matching on, or recovering from specific failure modes. Naming
//! error types `Error` without a subject prefix causes name collisions when re-exported at the
//! crate root or imported across modules, while manual `impl std::error::Error` + `Display`
//! boilerplate is verbose and error-prone compared to `#[derive(thiserror::Error)]`.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! #[derive(Debug)]
//! pub enum Error {
//!     NotFound,
//! }
//!
//! impl std::error::Error for Error {}
//!
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

use super::common::{
    TestScope, WithTestScope, derives_trait, extract_type_ident, has_suppression_attribute,
    path_ends_with_ident,
};
use crate::checkers::extract_fn_return_type;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule enforcing structured `<Subject>Error` types deriving `thiserror::Error` and forbidding unstructured string error returns.
pub(crate) struct ErrorTypesRule;

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
            test_scope: TestScope::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects function return types, error type definitions, and manual `Error` trait impls outside test scopes.
#[derive(WithTestScope)]
struct ErrorTypesVisitor<'a> {
    /// Lint context containing file path and coordinate mapping helpers.
    ctx: &'a LintContext<'a>,
    /// Accumulated diagnostic findings.
    diagnostics: Vec<Diagnostic>,
    /// Tracks active test scope across modules and test functions.
    test_scope: TestScope,
}

impl<'ast> Visit<'ast> for ErrorTypesVisitor<'_> {
    /// Tracks module scope and marks test scope active if annotated with `#[cfg(test)]`.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });
    }

    /// Inspects free functions and marks test scope active if annotated with `#[test]`.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        self.with_test_fn(&item_fn.attrs, |this| {
            visit::visit_item_fn(this, item_fn);
        });
    }

    /// Inspects methods in inherent or trait implementations and marks test scope active if annotated with `#[test]`.
    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        self.with_test_fn(&impl_fn.attrs, |this| {
            visit::visit_impl_item_fn(this, impl_fn);
        });
    }

    /// Inspects trait definition methods and marks test scope active if annotated with `#[test]`.
    fn visit_trait_item_fn(&mut self, trait_fn: &'ast syn::TraitItemFn) {
        self.with_test_fn(&trait_fn.attrs, |this| {
            visit::visit_trait_item_fn(this, trait_fn);
        });
    }

    /// Inspects struct definitions for error naming compliance outside test scopes.
    fn visit_item_struct(&mut self, item_struct: &'ast syn::ItemStruct) {
        if !self.test_scope.is_in_test()
            && !has_suppression_attribute(&item_struct.attrs, "purist::error_types")
            && let Some(diag) =
                check_error_type_naming(self.ctx, &item_struct.ident, &item_struct.attrs)
        {
            self.diagnostics.push(diag);
        }
        visit::visit_item_struct(self, item_struct);
    }

    /// Inspects enum definitions for error naming compliance outside test scopes.
    fn visit_item_enum(&mut self, item_enum: &'ast syn::ItemEnum) {
        if !self.test_scope.is_in_test()
            && !has_suppression_attribute(&item_enum.attrs, "purist::error_types")
            && let Some(diag) =
                check_error_type_naming(self.ctx, &item_enum.ident, &item_enum.attrs)
        {
            self.diagnostics.push(diag);
        }
        visit::visit_item_enum(self, item_enum);
    }

    /// Inspects `impl` blocks for manual `std::error::Error` implementations outside test scopes.
    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        if !self.test_scope.is_in_test()
            && !has_suppression_attribute(&item_impl.attrs, "purist::error_types")
            && let Some(diag) = check_manual_error_impl(self.ctx, item_impl)
        {
            self.diagnostics.push(diag);
        }
        visit::visit_item_impl(self, item_impl);
    }

    /// Inspects function signatures for unstructured string error return types outside test scopes.
    fn visit_signature(&mut self, sig: &'ast syn::Signature) {
        if !self.test_scope.is_in_test()
            && let Some(diag) = check_fn_signature_unstructured_error(self.ctx, sig)
        {
            self.diagnostics.push(diag);
        }
        visit::visit_signature(self, sig);
    }
}

/// Checks whether a struct or enum is named bare `Error` or derives `Error` without a `<Subject>Error` name.
fn check_error_type_naming(
    ctx: &LintContext<'_>,
    ident: &syn::Ident,
    attrs: &[syn::Attribute],
) -> Option<Diagnostic> {
    let name = ident.to_string();
    let derives_error = derives_trait(attrs, "Error");

    if name != "Error" && !derives_error {
        return None;
    }

    if is_subject_error_name(&name) {
        return None;
    }

    let span = ctx.to_span(ident.span());
    Some(
        Diagnostic::new(
            "purist::error_types",
            Severity::Warning,
            format!(
                "Error type '{name}' must follow the '<Subject>Error' naming convention (e.g., 'ConfigError', 'ParseError')."
            ),
        )
        .with_span(span)
        .with_suggested_fix(
            "Rename the error type to '<Subject>Error' so it remains unambiguous when imported or re-exported.",
        ),
    )
}

/// Returns true if `name` ends with `Error` and has a non-empty `<Subject>` prefix.
fn is_subject_error_name(name: &str) -> bool {
    name.strip_suffix("Error")
        .is_some_and(|prefix| !prefix.is_empty())
}

/// Checks whether an `impl` block manually implements `std::error::Error` (or `core::error::Error`).
fn check_manual_error_impl(ctx: &LintContext<'_>, item_impl: &syn::ItemImpl) -> Option<Diagnostic> {
    let (trait_path, _) = item_impl.trait_.as_ref()?;
    if !is_std_error_trait_path(trait_path) {
        return None;
    }

    let target_name = extract_type_ident(&item_impl.self_ty)
        .map_or_else(|| "error type".to_string(), ToString::to_string);
    let span = ctx.to_span(trait_path.span());

    Some(
        Diagnostic::new(
            "purist::error_types",
            Severity::Warning,
            format!(
                "Manual 'Error' trait implementation for '{target_name}'. Derive 'thiserror::Error' instead."
            ),
        )
        .with_span(span)
        .with_suggested_fix(
            "Replace manual 'impl std::error::Error' and 'Display' boilerplate with '#[derive(Debug, thiserror::Error)]'.",
        ),
    )
}

/// Returns true if `path` refers to `Error`, `error::Error`, `std::error::Error`, or `core::error::Error`.
fn is_std_error_trait_path(path: &syn::Path) -> bool {
    let segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    match segs.as_slice() {
        [last] => last == "Error",
        [penultimate, last] => penultimate == "error" && last == "Error",
        [first, penultimate, last] => {
            (first == "std" || first == "core") && penultimate == "error" && last == "Error"
        }
        _ => false,
    }
}

/// Inspects a function signature's return type and returns a diagnostic if it returns `Result<T, String>` or `Result<T, &str>`.
fn check_fn_signature_unstructured_error(
    ctx: &LintContext<'_>,
    sig: &syn::Signature,
) -> Option<Diagnostic> {
    let return_type = extract_fn_return_type(sig)?;

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

    #[googletest::test]
    fn bare_error_enum_name_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "#[derive(Debug, thiserror::Error)]\npub enum Error {\n    #[error(\"failed\")]\n    Failed,\n}\n";
        let ctx = LintContext::new(Path::new("src/dimacs.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ErrorTypesRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::error_types"));
        assert_that!(
            &diag.message,
            contains_substring(
                "Error type 'Error' must follow the '<Subject>Error' naming convention"
            )
        );
        Ok(())
    }

    #[googletest::test]
    fn derived_error_without_error_suffix_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "#[derive(Debug, thiserror::Error)]\npub enum ParseFailure {\n    #[error(\"failed\")]\n    Failed,\n}\n";
        let ctx = LintContext::new(Path::new("src/dimacs.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ErrorTypesRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::error_types"));
        assert_that!(
            &diag.message,
            contains_substring(
                "Error type 'ParseFailure' must follow the '<Subject>Error' naming convention"
            )
        );
        Ok(())
    }

    #[googletest::test]
    fn subject_error_enum_deriving_thiserror_is_permitted() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = "#[derive(Debug, thiserror::Error)]\npub enum DimacsError {\n    #[error(\"failed\")]\n    Failed,\n}\n";
        let ctx = LintContext::new(Path::new("src/dimacs.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ErrorTypesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn manual_std_error_impl_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub enum DimacsError { Failed }\nimpl std::error::Error for DimacsError {}\n";
        let ctx = LintContext::new(Path::new("src/dimacs.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ErrorTypesRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::error_types"));
        assert_that!(
            &diag.message,
            contains_substring(
                "Manual 'Error' trait implementation for 'DimacsError'. Derive 'thiserror::Error' instead."
            )
        );
        Ok(())
    }
}
