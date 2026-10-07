//! Rule: `purist::no_boxed_dyn_error`
//!
//! # What This Rule Does
//! Bans `Box<dyn std::error::Error>` (and trait object variants such as `Box<dyn Error + Send + Sync>`)
//! from function return types in production code. Binary `fn main()` and test scopes are exempt.
//!
//! # Why This Rule Exists
//! Dynamic trait objects like `Box<dyn Error>` erase concrete error identities, preventing callers from
//! matching variants, handling specific errors programmatically, or inspecting failure reasons. They
//! also mandate heap allocation on every error path and inhibit compiler optimizations. Production
//! libraries and modules should define concrete, typed error enums using `thiserror`.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! pub fn connect() -> Result<Connection, Box<dyn std::error::Error>> {
//!     // Type erased error prevents callers from recovering or matching specific errors
//!     ...
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! #[derive(Debug, thiserror::Error)]
//! pub enum ConnectionError {
//!     #[error("failed to resolve socket address: {0}")]
//!     ResolutionFailed(#[from] io::Error),
//!     #[error("handshake timed out")]
//!     Timeout,
//! }
//!
//! pub fn connect() -> Result<Connection, ConnectionError> {
//!     ...
//! }
//! ```

use super::common::{TestScopeTracker, path_ends_with_ident};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule banning `Box<dyn std::error::Error>` in production code return types.
pub struct NoBoxedDynErrorRule;

impl Rule for NoBoxedDynErrorRule {
    fn name(&self) -> &'static str {
        "purist::no_boxed_dyn_error"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = BoxedDynErrorVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScopeTracker::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects function signatures for `Box<dyn Error>` return types while tracking test scopes.
struct BoxedDynErrorVisitor<'a> {
    /// Lint context containing file path and coordinate mapping helpers.
    ctx: &'a LintContext<'a>,
    /// Accumulated diagnostic findings.
    diagnostics: Vec<Diagnostic>,
    /// Tracks active test scope across modules and test functions.
    test_scope: TestScopeTracker,
}

impl<'ast> Visit<'ast> for BoxedDynErrorVisitor<'_> {
    /// Tracks module scope and updates test status when entering `#[cfg(test)]` modules.
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let prev = self.test_scope.enter_mod(&item_mod.attrs);
        visit::visit_item_mod(self, item_mod);
        self.test_scope.exit_mod(prev);
    }

    /// Inspects free function return types and flags `Box<dyn Error>` if outside test scopes.
    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let prev = self.test_scope.enter_fn(&item_fn.attrs);
        if let Some(diag) = check_fn_signature(self.ctx, self.test_scope.is_in_test(), &item_fn.sig)
        {
            self.diagnostics.push(diag);
        }
        visit::visit_item_fn(self, item_fn);
        self.test_scope.exit_fn(prev);
    }

    /// Inspects methods in inherent or trait implementations and flags `Box<dyn Error>` if outside test scopes.
    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        let prev = self.test_scope.enter_fn(&impl_fn.attrs);
        if let Some(diag) = check_fn_signature(self.ctx, self.test_scope.is_in_test(), &impl_fn.sig)
        {
            self.diagnostics.push(diag);
        }
        visit::visit_impl_item_fn(self, impl_fn);
        self.test_scope.exit_fn(prev);
    }
}

/// Inspects a function signature and returns a diagnostic if it returns `Box<dyn Error>`.
fn check_fn_signature(
    ctx: &LintContext<'_>,
    in_test: bool,
    sig: &syn::Signature,
) -> Option<Diagnostic> {
    if in_test {
        return None;
    }

    let fn_name = sig.ident.to_string();

    // Exempt main in binary entry points
    if fn_name == "main" {
        return None;
    }

    let syn::ReturnType::Type(_, ty) = &sig.output else {
        return None;
    };

    if !contains_boxed_dyn_error(ty) {
        return None;
    }

    let span = ctx.to_span(sig.output.span());
    Some(
        Diagnostic::new(
            "purist::no_boxed_dyn_error",
            Severity::Error,
            format!(
                "Function '{fn_name}' returns 'Box<dyn Error>'. Return a concrete domain error enum deriving 'thiserror::Error' instead."
            ),
        )
        .with_span(span)
        .with_suggested_fix("Return a concrete domain error enum deriving 'thiserror::Error' with appropriate '#[from]' conversions."),
    )
}

/// Recursively detects whether a type contains `Box<dyn Error>` or any trait object targeting `Error`.
fn contains_boxed_dyn_error(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(type_path) => type_path
            .path
            .segments
            .iter()
            .any(segment_contains_boxed_dyn_error),
        syn::Type::Tuple(tup) => tup.elems.iter().any(contains_boxed_dyn_error),
        syn::Type::Reference(r) => contains_boxed_dyn_error(&r.elem),
        _ => false,
    }
}

/// Checks whether a single path segment holds Box<dyn Error> or nested error types.
fn segment_contains_boxed_dyn_error(segment: &syn::PathSegment) -> bool {
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return false;
    };

    let is_box = segment.ident == "Box";
    for arg in &args.args {
        let syn::GenericArgument::Type(inner_ty) = arg else {
            continue;
        };
        if (is_box && is_dyn_error_trait(inner_ty)) || contains_boxed_dyn_error(inner_ty) {
            return true;
        }
    }
    false
}

/// Returns true if the type is a trait object or path referencing `Error`.
fn is_dyn_error_trait(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::TraitObject(to) => to.bounds.iter().any(|bound| match bound {
            syn::TypeParamBound::Trait(trait_bound) => {
                path_ends_with_ident(&trait_bound.path, "Error")
            }
            _ => false,
        }),
        syn::Type::Path(tp) => path_ends_with_ident(&tp.path, "Error"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn boxed_dyn_error_in_fn_signature_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn run() -> Result<(), Box<dyn std::error::Error>> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/engine.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBoxedDynErrorRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_boxed_dyn_error"));
        assert_that!(
            &diag.message,
            contains_substring("Function 'run' returns 'Box<dyn Error>'")
        );
        Ok(())
    }

    #[googletest::test]
    fn boxed_dyn_error_with_send_sync_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn execute() -> Result<String, Box<dyn std::error::Error + Send + Sync>> { Ok(String::new()) }\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBoxedDynErrorRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(diag.severity, eq(Severity::Error));
        Ok(())
    }

    #[googletest::test]
    fn concrete_domain_error_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn run() -> Result<(), ConfigError> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/engine.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBoxedDynErrorRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn boxed_dyn_error_in_main_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "fn main() -> Result<(), Box<dyn std::error::Error>> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBoxedDynErrorRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn boxed_dyn_error_in_test_fn_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source =
            "#[test]\nfn run_test() -> Result<(), Box<dyn std::error::Error>> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBoxedDynErrorRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
