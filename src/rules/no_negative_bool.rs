//! Rule: `purist::no_negative_bool`
//!
//! # What This Rule Does
//! Flags boolean fields, enum variant fields, and variables that use negative naming
//! prefixes such as `skip_*`, `no_*`, `not_*`, `without_*`, `disable_*`, `disabled_*`, or `disallow_*`,
//! as well as exact negative words such as `skip`, `disable`, and `disabled`.
//!
//! This includes:
//! 1. Struct fields of type `bool` or `Option<bool>` (e.g. `skip_fmt: bool`, `no_cache: bool`, `disabled: bool`).
//! 2. Enum variant fields of boolean type (e.g. `Variant { skip_check: bool }`).
//! 3. Local `let` bindings of boolean type (e.g. `let skip = true;`, `let no_color: bool = false;`).
//!
//! # Why This Rule Exists
//! Negative boolean naming produces confusing double negatives in code, configuration,
//! and conditional logic (e.g. `if !skip_cache`, `disabled: false`).
//! Data structures and variables should always be affirmative and positive.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! pub struct ServerConfig {
//!     skip_auth: bool, // Negative boolean field
//!     no_cache: bool,  // Negative boolean field
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! pub struct ServerConfig {
//!     auth_required: bool, // Affirmative boolean
//!     cache: bool,         // Affirmative boolean
//! }
//! ```

use super::common::{TestScopeTracker, is_bool_type};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use crate::trackers::TypeScopeTracker;
use syn::visit::{self, Visit};

const NEGATIVE_PREFIXES: &[&str] = &[
    "skip_",
    "no_",
    "not_",
    "without_",
    "disable_",
    "disabled_",
    "disallow_",
];

const NEGATIVE_EXACT_WORDS: &[&str] = &["skip", "disable", "disabled", "disallow"];

/// Rule enforcing affirmative/positive boolean naming across structs, enums, and bindings.
pub struct NoNegativeBoolRule;

impl Rule for NoNegativeBoolRule {
    fn name(&self) -> &'static str {
        "purist::no_negative_bool"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut visitor = NegativeBoolVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScopeTracker::new(ctx.is_test_file()),
            type_scope: TypeScopeTracker::new(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

struct NegativeBoolVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScopeTracker,
    type_scope: TypeScopeTracker,
}

impl<'ast> Visit<'ast> for NegativeBoolVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.test_scope.push_mod(&item_mod.attrs);
        visit::visit_item_mod(self, item_mod);
        self.test_scope.pop();
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        self.test_scope.push_fn(&item_fn.attrs);
        visit::visit_item_fn(self, item_fn);
        self.test_scope.pop();
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        self.test_scope.push_fn(&impl_fn.attrs);
        visit::visit_impl_item_fn(self, impl_fn);
        self.test_scope.pop();
    }

    fn visit_item_struct(&mut self, item_struct: &'ast syn::ItemStruct) {
        self.type_scope.push_struct(&item_struct.ident);
        visit::visit_item_struct(self, item_struct);
        self.type_scope.pop();
    }

    fn visit_item_enum(&mut self, item_enum: &'ast syn::ItemEnum) {
        self.type_scope.push_enum(&item_enum.ident);
        visit::visit_item_enum(self, item_enum);
        self.type_scope.pop();
    }

    fn visit_variant(&mut self, variant: &'ast syn::Variant) {
        self.type_scope.push_variant(&variant.ident);
        visit::visit_variant(self, variant);
        self.type_scope.pop();
    }

    fn visit_field(&mut self, field: &'ast syn::Field) {
        if !self.test_scope.is_in_test() {
            self.check_field_negative_bool(field);
        }
        visit::visit_field(self, field);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if !self.test_scope.is_in_test() {
            self.check_local_binding_negative_bool(local);
        }
        visit::visit_local(self, local);
    }
}

impl NegativeBoolVisitor<'_> {
    fn check_field_negative_bool(&mut self, field: &syn::Field) {
        let Some(ident) = &field.ident else {
            return;
        };
        let field_name = ident.to_string();

        if is_bool_type(&field.ty)
            && let Some(prefix) = negative_prefix_or_word(&field_name)
        {
            let span = self.ctx.to_span(ident.span());
            let suggested = suggest_affirmative(&field_name, prefix);
            let container_desc = self
                .type_scope
                .container_description()
                .unwrap_or_else(|| "type".to_string());
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::no_negative_bool",
                    Severity::Warning,
                    format!(
                        "Field '{field_name}' in {container_desc} uses negative boolean naming. Use affirmative/positive naming (e.g. '{suggested}') instead of negative booleans."
                    ),
                )
                .with_span(span)
                .with_suggested_fix(format!(
                    "Rename '{field_name}' to affirmative '{suggested}'."
                )),
            );
        }
    }

    fn check_local_binding_negative_bool(&mut self, local: &syn::Local) {
        let (pat_ident, explicit_ty) = match &local.pat {
            syn::Pat::Ident(pat_ident) => (pat_ident, None),
            syn::Pat::Type(pat_type) => {
                if let syn::Pat::Ident(ref pat_ident) = *pat_type.pat {
                    (pat_ident, Some(&*pat_type.ty))
                } else {
                    return;
                }
            }
            _ => return,
        };
        let var_name = pat_ident.ident.to_string();

        let Some(prefix) = negative_prefix_or_word(&var_name) else {
            return;
        };

        let is_bool = if let Some(ty) = explicit_ty {
            is_bool_type(ty)
        } else {
            local
                .init
                .as_ref()
                .is_some_and(|init| is_bool_expr(&init.expr))
        };

        if is_bool {
            let span = self.ctx.to_span(pat_ident.ident.span());
            let suggested = suggest_affirmative(&var_name, prefix);
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::no_negative_bool",
                    Severity::Warning,
                    format!(
                        "Variable '{var_name}' uses negative boolean naming. Use affirmative/positive naming (e.g. '{suggested}') instead of negative booleans."
                    ),
                )
                .with_span(span)
                .with_suggested_fix(format!(
                    "Rename '{var_name}' to affirmative '{suggested}'."
                )),
            );
        }
    }
}

fn is_bool_expr(expr: &syn::Expr) -> bool {
    matches!(
        expr,
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Bool(_),
            ..
        }) | syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Not(_),
            ..
        })
    )
}

fn negative_prefix_or_word(name: &str) -> Option<&'static str> {
    if let Some(&word) = NEGATIVE_EXACT_WORDS.iter().find(|&&word| name == word) {
        return Some(word);
    }
    NEGATIVE_PREFIXES
        .iter()
        .find(|&&prefix| name.starts_with(prefix))
        .copied()
}

fn suggest_affirmative(name: &str, prefix: &str) -> String {
    if NEGATIVE_EXACT_WORDS.contains(&prefix) {
        return "enabled".to_string();
    }
    name.strip_prefix(prefix)
        .filter(|s| !s.is_empty())
        .unwrap_or("enabled")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn affirmative_bool_fields_are_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            pub struct CheckCommand {
                pub fmt: bool,
                pub clippy: bool,
                pub quiet: bool,
            }
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/command.rs"), code);
        let rule = NoNegativeBoolRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn skip_prefix_field_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            pub struct CheckCommand {
                skip_fmt: bool,
            }
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/command.rs"), code);
        let rule = NoNegativeBoolRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(diag.message, contains_substring("skip_fmt"));
        assert_that!(
            diag.message,
            contains_substring("uses negative boolean naming")
        );
        Ok(())
    }

    #[googletest::test]
    fn no_prefix_field_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            pub struct Cli {
                no_cache: bool,
            }
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/cli.rs"), code);
        let rule = NoNegativeBoolRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(diag.message, contains_substring("no_cache"));
        Ok(())
    }

    #[googletest::test]
    fn non_clap_struct_with_negative_bool_field_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            pub struct ServerConfig {
                pub skip_tls: bool,
                pub no_cache: Option<bool>,
            }
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/config.rs"), code);
        let rule = NoNegativeBoolRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags.len(), eq(2));
        let first = diags.first().ok_or("expected first diagnostic")?;
        let second = diags.get(1).ok_or("expected second diagnostic")?;
        assert_that!(first.message, contains_substring("skip_tls"));
        assert_that!(second.message, contains_substring("no_cache"));
        Ok(())
    }

    #[googletest::test]
    fn struct_with_exact_skip_or_disabled_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            pub struct Service {
                pub disabled: bool,
                pub skip: bool,
            }
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/service.rs"), code);
        let rule = NoNegativeBoolRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags.len(), eq(2));
        let first = diags.first().ok_or("expected first diagnostic")?;
        let second = diags.get(1).ok_or("expected second diagnostic")?;
        assert_that!(first.message, contains_substring("disabled"));
        assert_that!(second.message, contains_substring("skip"));
        Ok(())
    }

    #[googletest::test]
    fn non_boolean_skip_field_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            pub struct InternalState {
                pub skip_count: usize,
                pub skip_reason: String,
            }
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/state.rs"), code);
        let rule = NoNegativeBoolRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn affirmative_bool_fields_with_options_are_permitted() -> Result<(), Box<dyn std::error::Error>>
    {
        let code = r#"
            pub struct ClientConfig {
                pub cache: bool,
                pub tls_enabled: bool,
                pub auto_retry: Option<bool>,
            }
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/client.rs"), code);
        let rule = NoNegativeBoolRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn enum_variant_with_negative_bool_field_is_flagged() -> Result<(), Box<dyn std::error::Error>>
    {
        let code = r#"
            pub enum Action {
                Run { skip_check: bool },
            }
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/action.rs"), code);
        let rule = NoNegativeBoolRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(diag.message, contains_substring("skip_check"));
        Ok(())
    }

    #[googletest::test]
    fn local_variable_with_negative_bool_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            fn execute() {
                let skip_formatting = true;
                let no_lints: bool = false;
            }
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/exec.rs"), code);
        let rule = NoNegativeBoolRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags.len(), eq(2));
        let first = diags.first().ok_or("expected first diagnostic")?;
        let second = diags.get(1).ok_or("expected second diagnostic")?;
        assert_that!(first.message, contains_substring("skip_formatting"));
        assert_that!(second.message, contains_substring("no_lints"));
        Ok(())
    }

    #[googletest::test]
    fn local_variable_with_affirmative_bool_is_permitted() -> Result<(), Box<dyn std::error::Error>>
    {
        let code = r#"
            fn execute() {
                let formatting_enabled = true;
                let run_lints: bool = false;
            }
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/exec.rs"), code);
        let rule = NoNegativeBoolRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn cfg_test_module_is_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            #[cfg(test)]
            mod tests {
                pub struct TestOpts {
                    pub skip_something: bool,
                }
            }
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), code);
        let rule = NoNegativeBoolRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags, is_empty());
        Ok(())
    }
}
