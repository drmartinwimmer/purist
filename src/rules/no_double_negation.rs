//! # Rule: purist::no_double_negation
//!
//! ## What This Rule Does
//! Flags function and method signatures that use negative boolean naming, creating
//! double negations at call sites or in conditional logic.
//!
//! This includes:
//! 1. Methods taking a `bool` (or `Option<bool>`) parameter whose name starts with negative prefixes such as
//!    `with_skip_*`, `set_skip_*`, `skip_*`, `with_no_*`, `with_not_*`, `with_disable_*`,
//!    `with_without_*`, `with_exclude_*`, `with_disallow_*`, `with_ignore_*`.
//! 2. Parameters of type `bool` (or `Option<bool>`) named with negative terms such as
//!    `skip`, `disable`, `disabled`, `no_*`, `not_*`, `without_*`, `exclude_*`, `disallow_*`.
//! 3. Predicates/getters returning `bool` (or `Option<bool>`) named with negative terms such as
//!    `is_skip_*`, `is_not_*`, `is_no_*`, `is_disabled_*`, `is_without_*`, `has_no_*`, `has_not_*`.
//!
//! ## Why This Rule Exists
//! Negative boolean naming leads to awkward and error-prone double negatives when negated or
//! when passing `false`. For example:
//! - `cmd.with_skip_fmt(false)` means "do not skip formatting" (double negation).
//! - `if !cmd.is_skip_fmt()` means "if not skip" (double negation).
//! - `run(false)` where parameter is `skip: bool` means "do not skip".
//!
//! APIs should always prefer affirmative naming:
//! - Use `with_fmt(enabled: bool)` instead of `with_skip_fmt(skip: bool)`.
//! - Use `is_fmt_enabled(&self) -> bool` instead of `is_skip_fmt(&self) -> bool`.
//! - Use `enabled: bool` or `include: bool` instead of `skip: bool` or `disable: bool`.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! pub struct Command;
//!
//! impl Command {
//!     // Method taking bool with negative prefix
//!     pub fn with_skip_fmt(mut self, skip: bool) -> Self {
//!         self
//!     }
//!
//!     // Predicate returning bool with negative prefix
//!     pub fn is_skip_fmt(&self) -> bool {
//!         false
//!     }
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! pub struct Command;
//!
//! impl Command {
//!     // Affirmative builder method and parameter name
//!     pub fn with_fmt(mut self, enabled: bool) -> Self {
//!         self
//!     }
//!
//!     // Affirmative predicate
//!     pub fn is_fmt_enabled(&self) -> bool {
//!         true
//!     }
//! }
//! ```

use super::common::{TestScopeTracker, path_ends_with_ident};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::visit::{self, Visit};
use syn::{FnArg, Pat, ReturnType, Type};

/// Rule forbidding negative boolean identifiers and double negation patterns.
pub struct NoDoubleNegationRule;

impl Rule for NoDoubleNegationRule {
    fn name(&self) -> &'static str {
        "purist::no_double_negation"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut visitor = DoubleNegationVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScopeTracker::new(ctx.is_test_file()),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that inspects function signatures for negative boolean naming patterns.
struct DoubleNegationVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScopeTracker,
}

impl<'ast> Visit<'ast> for DoubleNegationVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let prev = self.test_scope.enter_mod(&item_mod.attrs);
        visit::visit_item_mod(self, item_mod);
        self.test_scope.exit_mod(prev);
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let prev = self.test_scope.enter_fn(&item_fn.attrs);
        if !self.test_scope.is_in_test() {
            self.check_signature(&item_fn.sig);
        }

        visit::visit_item_fn(self, item_fn);
        self.test_scope.exit_fn(prev);
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        let prev = self.test_scope.enter_fn(&impl_fn.attrs);
        if !self.test_scope.is_in_test() {
            self.check_signature(&impl_fn.sig);
        }

        visit::visit_impl_item_fn(self, impl_fn);
        self.test_scope.exit_fn(prev);
    }

    fn visit_trait_item_fn(&mut self, trait_fn: &'ast syn::TraitItemFn) {
        let prev = self.test_scope.enter_fn(&trait_fn.attrs);
        if !self.test_scope.is_in_test() {
            self.check_signature(&trait_fn.sig);
        }

        visit::visit_trait_item_fn(self, trait_fn);
        self.test_scope.exit_fn(prev);
    }
}

impl DoubleNegationVisitor<'_> {
    fn check_signature(&mut self, sig: &syn::Signature) {
        let fn_name = sig.ident.to_string();

        let has_bool_param = sig.inputs.iter().any(|arg| {
            if let FnArg::Typed(pat_type) = arg {
                is_bool_type(&pat_type.ty)
            } else {
                false
            }
        });

        // 1. Check method name taking a bool argument
        if has_bool_param && is_negative_method_name(&fn_name) {
            let span = self.ctx.to_span(sig.ident.span());
            let suggested = suggest_affirmative_method_name(&fn_name);
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::no_double_negation",
                    Severity::Warning,
                    format!(
                        "Method '{fn_name}' uses negative boolean naming causing double negation (e.g. '{fn_name}(false)'). Use affirmative naming instead."
                    ),
                )
                .with_span(span)
                .with_suggested_fix(format!(
                    "Rename to an affirmative identifier (e.g. '{suggested}')."
                )),
            );
        }

        // 2. Check individual boolean parameters
        for arg in &sig.inputs {
            if let FnArg::Typed(pat_type) = arg
                && is_bool_type(&pat_type.ty)
                && let Pat::Ident(pat_ident) = &*pat_type.pat
            {
                let param_name = pat_ident.ident.to_string();
                if is_negative_parameter_name(&param_name) {
                    let span = self.ctx.to_span(pat_ident.ident.span());
                    self.diagnostics.push(
                        Diagnostic::new(
                            "purist::no_double_negation",
                            Severity::Warning,
                            format!(
                                "Parameter '{param_name}' uses negative boolean naming causing double negation when passed 'false'. Use affirmative naming instead (e.g. 'enabled', 'include')."
                            ),
                        )
                        .with_span(span)
                        .with_suggested_fix(
                            "Rename to an affirmative parameter such as 'enabled' or 'include'.",
                        ),
                    );
                }
            }
        }

        // 3. Check predicate/getter returning bool
        if returns_bool(&sig.output) && is_negative_predicate_name(&fn_name) {
            let span = self.ctx.to_span(sig.ident.span());
            let suggested = suggest_affirmative_predicate_name(&fn_name);
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::no_double_negation",
                    Severity::Warning,
                    format!(
                        "Predicate '{fn_name}' uses negative boolean naming causing double negation when negated (e.g. '!{fn_name}()'). Use affirmative naming instead."
                    ),
                )
                .with_span(span)
                .with_suggested_fix(format!(
                    "Rename to an affirmative predicate (e.g. '{suggested}')."
                )),
            );
        }
    }
}

/// Checks whether a type is `bool` or `Option<bool>`.
fn is_bool_type(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => {
            if path_ends_with_ident(&type_path.path, "bool") {
                return true;
            }
            if path_ends_with_ident(&type_path.path, "Option")
                && let Some(segment) = type_path.path.segments.last()
                && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
                && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
            {
                return is_bool_type(inner_ty);
            }
            false
        }
        _ => false,
    }
}

/// Checks whether a return type is `bool` or `Option<bool>`.
fn returns_bool(output: &ReturnType) -> bool {
    match output {
        ReturnType::Default => false,
        ReturnType::Type(_, ty) => is_bool_type(ty),
    }
}

/// Negative prefixes for methods taking boolean parameters.
const NEGATIVE_METHOD_PREFIXES: &[&str] = &[
    "with_skip_",
    "set_skip_",
    "skip_",
    "with_no_",
    "set_no_",
    "no_",
    "with_not_",
    "set_not_",
    "not_",
    "with_without_",
    "set_without_",
    "without_",
    "with_disable_",
    "with_disabled_",
    "set_disable_",
    "set_disabled_",
    "disable_",
    "disabled_",
    "with_disallow_",
    "set_disallow_",
    "disallow_",
    "with_exclude_",
    "set_exclude_",
    "exclude_",
    "with_ignore_",
    "set_ignore_",
    "ignore_",
];

/// Checks if a method name starts with any negative prefix.
fn is_negative_method_name(name: &str) -> bool {
    NEGATIVE_METHOD_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// Suggests an affirmative method name replacement.
fn suggest_affirmative_method_name(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("with_skip_") {
        format!("with_{rest}")
    } else if let Some(rest) = name.strip_prefix("set_skip_") {
        format!("set_{rest}")
    } else if let Some(rest) = name.strip_prefix("with_no_") {
        format!("with_{rest}")
    } else if let Some(rest) = name.strip_prefix("with_disable_") {
        format!("with_{rest}")
    } else if let Some(rest) = name.strip_prefix("with_disabled_") {
        format!("with_{rest}")
    } else if let Some(rest) = name.strip_prefix("with_without_") {
        format!("with_with_{rest}")
    } else {
        "with_enabled".to_string()
    }
}

/// Negative names or prefixes for boolean parameters.
const NEGATIVE_PARAM_EXACT: &[&str] = &[
    "skip", "disable", "disabled", "exclude", "disallow", "ignore", "without",
];

const NEGATIVE_PARAM_PREFIXES: &[&str] = &[
    "skip_",
    "no_",
    "not_",
    "without_",
    "disable_",
    "disabled_",
    "exclude_",
    "disallow_",
    "ignore_",
];

/// Checks if a parameter name is negative.
fn is_negative_parameter_name(name: &str) -> bool {
    NEGATIVE_PARAM_EXACT.contains(&name)
        || NEGATIVE_PARAM_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// Negative prefixes for predicate/getter functions returning bool.
const NEGATIVE_PREDICATE_PREFIXES: &[&str] = &[
    "is_skip_",
    "is_not_",
    "is_no_",
    "is_without_",
    "is_disable_",
    "is_disabled_",
    "is_disallow_",
    "is_disallowed_",
    "is_exclude_",
    "is_excluded_",
    "has_no_",
    "has_not_",
];

/// Checks if a predicate name starts with any negative prefix.
fn is_negative_predicate_name(name: &str) -> bool {
    NEGATIVE_PREDICATE_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// Suggests an affirmative predicate replacement.
fn suggest_affirmative_predicate_name(name: &str) -> String {
    if let Some(rest) = name.strip_prefix("is_skip_") {
        format!("is_{rest}_enabled")
    } else if let Some(rest) = name.strip_prefix("is_not_") {
        format!("is_{rest}")
    } else if let Some(rest) = name.strip_prefix("is_disabled_") {
        format!("is_{rest}_enabled")
    } else if let Some(rest) = name.strip_prefix("is_no_") {
        format!("is_{rest}")
    } else {
        "is_enabled".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn with_skip_method_taking_bool_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Command;
impl Command {
    pub fn with_skip_fmt(mut self, enabled: bool) -> Self {
        self
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoDoubleNegationRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_double_negation"));
        assert_that!(
            &diag.message,
            contains_substring("Method 'with_skip_fmt' uses negative boolean naming")
        );
        Ok(())
    }

    #[googletest::test]
    fn is_skip_predicate_returning_bool_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Command {
    skip_fmt: bool,
}
impl Command {
    pub fn is_skip_fmt(&self) -> bool {
        self.skip_fmt
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoDoubleNegationRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_double_negation"));
        assert_that!(
            &diag.message,
            contains_substring("Predicate 'is_skip_fmt' uses negative boolean naming")
        );
        Ok(())
    }

    #[googletest::test]
    fn parameter_with_negative_name_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn configure(skip: bool) {}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoDoubleNegationRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_double_negation"));
        assert_that!(
            &diag.message,
            contains_substring("Parameter 'skip' uses negative boolean naming")
        );
        Ok(())
    }

    #[googletest::test]
    fn with_disable_method_taking_bool_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Config;
impl Config {
    pub fn with_disable_cache(mut self, enabled: bool) -> Self {
        self
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoDoubleNegationRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_double_negation"));
        assert_that!(
            &diag.message,
            contains_substring("Method 'with_disable_cache' uses negative boolean naming")
        );
        Ok(())
    }

    #[googletest::test]
    fn trait_method_taking_bool_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub trait Builder {
    fn with_skip_fmt(self, enabled: bool) -> Self;
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoDoubleNegationRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_double_negation"));
        assert_that!(
            &diag.message,
            contains_substring("Method 'with_skip_fmt' uses negative boolean naming")
        );
        Ok(())
    }

    #[googletest::test]
    fn method_with_negative_name_and_negative_param_flags_both()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Command;
impl Command {
    pub fn with_skip_fmt(mut self, skip: bool) -> Self {
        self
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoDoubleNegationRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(2));
        Ok(())
    }

    #[googletest::test]
    fn affirmative_boolean_naming_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Command;
impl Command {
    pub fn with_fmt(mut self, enabled: bool) -> Self {
        self
    }
    pub fn is_fmt_enabled(&self) -> bool {
        true
    }
    pub fn skip_whitespace(&mut self) {}
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoDoubleNegationRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn function_in_test_module_is_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[cfg(test)]
mod tests {
    pub fn with_skip_fmt(skip: bool) {}
    pub fn is_skip_fmt() -> bool { false }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoDoubleNegationRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn function_with_test_attribute_is_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn is_skip_fmt() -> bool {
    false
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoDoubleNegationRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn integration_test_file_is_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn with_skip_fmt(skip: bool) {}
"#;
        let ctx = LintContext::new(Path::new("tests/integration_test.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoDoubleNegationRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
