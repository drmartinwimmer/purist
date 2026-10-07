//! Rule: `purist::no_negative_bool`
//!
//! # What This Rule Does
//! Flags boolean fields, CLI argument flags, and variables that use negative naming
//! prefixes such as `skip_*`, `no_*`, `not_*`, `without_*`, `disable_*`, `disabled_*`, or `disallow_*`,
//! as well as exact negative words such as `skip`, `disable`, and `disabled`.
//!
//! This includes:
//! 1. Struct fields of type `bool` or `Option<bool>` (e.g. `skip_fmt: bool`, `no_cache: bool`, `disabled: bool`).
//! 2. Clap CLI argument flags in `#[arg(long = "...")]` or `#[arg(alias = "...")]` (e.g. `long = "skip-fmt"`).
//! 3. Enum variant fields of boolean type (e.g. `Variant { skip_check: bool }`).
//! 4. Local `let` bindings of boolean type (e.g. `let skip = true;`, `let no_color: bool = false;`).
//!
//! # Why This Rule Exists
//! Negative boolean naming produces confusing double negatives in code, configuration,
//! conditional logic, and CLI flags (e.g. `if !skip_cache`, `--skip-fmt false`, `disabled: false`).
//! Data structures, CLI arguments, and variables should always be affirmative and positive.
//! For features that are enabled by default, the flag or setting should default to true
//! (e.g. `#[arg(long, default_value_t = true, action = clap::ArgAction::Set)]`).
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! pub struct ServerConfig {
//!     skip_auth: bool, // Negative boolean field
//!     no_cache: bool,  // Negative boolean field
//! }
//!
//! #[derive(clap::Args)]
//! pub struct CheckCommand {
//!     #[arg(long)]
//!     skip_fmt: bool, // Negative CLI flag
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! pub struct ServerConfig {
//!     auth_required: bool, // Affirmative boolean
//!     cache: bool,         // Affirmative boolean
//! }
//!
//! #[derive(clap::Args)]
//! pub struct CheckCommand {
//!     /// Run cargo fmt checks (enabled by default; pass false to skip)
//!     #[arg(
//!         long,
//!         default_value_t = true,
//!         action = clap::ArgAction::Set,
//!         num_args(0..=1),
//!         default_missing_value = "true"
//!     )]
//!     fmt: bool, // Affirmative flag defaulting to true
//! }
//! ```

use super::common::{TestScopeTracker, path_ends_with_ident};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use syn::Type;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

const NEGATIVE_PREFIXES: &[&str] = &[
    "skip_",
    "skip-",
    "no_",
    "no-",
    "not_",
    "not-",
    "without_",
    "without-",
    "disable_",
    "disable-",
    "disabled_",
    "disabled-",
    "disallow_",
    "disallow-",
];

const NEGATIVE_EXACT_WORDS: &[&str] = &["skip", "disable", "disabled", "disallow"];

/// Rule enforcing affirmative/positive boolean naming across structs, CLI flags, and bindings.
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
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

struct NegativeBoolVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScopeTracker,
}

impl<'ast> Visit<'ast> for NegativeBoolVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let prev = self.test_scope.enter_mod(&item_mod.attrs);
        visit::visit_item_mod(self, item_mod);
        self.test_scope.exit_mod(prev);
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let prev = self.test_scope.enter_fn(&item_fn.attrs);
        visit::visit_item_fn(self, item_fn);
        self.test_scope.exit_fn(prev);
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        let prev = self.test_scope.enter_fn(&impl_fn.attrs);
        visit::visit_impl_item_fn(self, impl_fn);
        self.test_scope.exit_fn(prev);
    }

    fn visit_item_struct(&mut self, item_struct: &'ast syn::ItemStruct) {
        if !self.test_scope.is_in_test() {
            self.check_struct(item_struct);
        }
        visit::visit_item_struct(self, item_struct);
    }

    fn visit_item_enum(&mut self, item_enum: &'ast syn::ItemEnum) {
        if !self.test_scope.is_in_test() {
            self.check_enum(item_enum);
        }
        visit::visit_item_enum(self, item_enum);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if !self.test_scope.is_in_test() {
            self.check_local(local);
        }
        visit::visit_local(self, local);
    }
}

impl NegativeBoolVisitor<'_> {
    fn check_struct(&mut self, item_struct: &syn::ItemStruct) {
        let struct_name = item_struct.ident.to_string();
        let is_clap = derives_clap(&item_struct.attrs);

        let fields = match &item_struct.fields {
            syn::Fields::Named(named) => &named.named,
            _ => return,
        };

        for field in fields {
            let Some(ident) = &field.ident else {
                continue;
            };
            let field_name = ident.to_string();

            let is_bool = is_bool_type(&field.ty);

            // 1. Check field identifier if it's a bool or Clap flag
            if (is_bool || is_clap)
                && let Some(prefix) = negative_prefix_or_word(&field_name)
            {
                let span = self.ctx.to_span(field.ident.span());
                let suggested = suggest_affirmative(&field_name, prefix);
                let message = if is_clap {
                    format!(
                        "CLI argument '{field_name}' in struct '{struct_name}' uses negative boolean naming. Use affirmative/positive flags (e.g. defaulting to true) instead of negative flags."
                    )
                } else {
                    format!(
                        "Field '{field_name}' in struct '{struct_name}' uses negative boolean naming. Use affirmative/positive naming (e.g. '{suggested}') instead of negative booleans."
                    )
                };
                let fix = if is_clap {
                    format!(
                        "Rename '{field_name}' to an affirmative flag (e.g. '{suggested}') and default to true (e.g. #[arg(long, default_value_t = true, action = clap::ArgAction::Set)])."
                    )
                } else {
                    format!("Rename '{field_name}' to affirmative '{suggested}'.")
                };
                self.diagnostics.push(
                    Diagnostic::new("purist::no_negative_bool", Severity::Warning, message)
                        .with_span(span)
                        .with_suggested_fix(fix),
                );
                continue;
            }

            // 2. Check explicit attributes on #[arg(...)]
            self.check_arg_attributes(&field.attrs, &struct_name);
        }
    }

    fn check_arg_attributes(&mut self, attrs: &[syn::Attribute], struct_name: &str) {
        for attr in attrs {
            if !attr.path().is_ident("arg") {
                continue;
            }

            let mut flagged_attr_name = None;
            drop(attr.parse_nested_meta(|meta| {
                if (meta.path.is_ident("long") || meta.path.is_ident("alias"))
                    && let Ok(value) = meta.value()
                    && let Ok(syn::Lit::Str(s)) = value.parse::<syn::Lit>()
                {
                    let val = s.value();
                    if let Some(prefix) = negative_prefix_or_word(&val) {
                        flagged_attr_name = Some((val, prefix));
                    }
                }
                Ok(())
            }));

            if let Some((attr_name, prefix)) = flagged_attr_name {
                let span = self.ctx.to_span(attr.span());
                let suggested = suggest_affirmative(&attr_name, prefix);
                self.diagnostics.push(
                    Diagnostic::new(
                        "purist::no_negative_bool",
                        Severity::Warning,
                        format!(
                            "CLI argument flag '{attr_name}' in struct '{struct_name}' uses negative naming. Use affirmative/positive flags instead."
                        ),
                    )
                    .with_span(span)
                    .with_suggested_fix(format!(
                        "Rename '{attr_name}' to affirmative '{suggested}'."
                    )),
                );
            }
        }
    }

    fn check_enum(&mut self, item_enum: &syn::ItemEnum) {
        let enum_name = item_enum.ident.to_string();

        for variant in &item_enum.variants {
            let variant_name = variant.ident.to_string();
            let fields = match &variant.fields {
                syn::Fields::Named(named) => &named.named,
                _ => continue,
            };

            for field in fields {
                let Some(ident) = &field.ident else {
                    continue;
                };
                let field_name = ident.to_string();

                if is_bool_type(&field.ty)
                    && let Some(prefix) = negative_prefix_or_word(&field_name)
                {
                    let span = self.ctx.to_span(field.ident.span());
                    let suggested = suggest_affirmative(&field_name, prefix);
                    self.diagnostics.push(
                        Diagnostic::new(
                            "purist::no_negative_bool",
                            Severity::Warning,
                            format!(
                                "Field '{field_name}' in enum variant '{enum_name}::{variant_name}' uses negative boolean naming. Use affirmative/positive naming (e.g. '{suggested}') instead."
                            ),
                        )
                        .with_span(span)
                        .with_suggested_fix(format!(
                            "Rename '{field_name}' to affirmative '{suggested}'."
                        )),
                    );
                }
            }
        }
    }

    fn check_local(&mut self, local: &syn::Local) {
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

        // Check if the local variable is of boolean type or initialized with boolean
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
                        "Variable '{var_name}' uses negative boolean naming. Use affirmative/positive naming (e.g. '{suggested}') instead."
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

/// Checks whether an attribute list includes a derive for `Args` or `Parser`.
fn derives_clap(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let mut found_clap = false;
        drop(attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("Args")
                || meta.path.is_ident("Parser")
                || meta
                    .path
                    .segments
                    .iter()
                    .any(|seg| seg.ident == "Args" || seg.ident == "Parser")
            {
                found_clap = true;
            }
            Ok(())
        }));
        found_clap
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn affirmative_clap_flags_are_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            #[derive(clap::Args)]
            pub struct CheckCommand {
                #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
                fmt: bool,
                #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
                clippy: bool,
                #[arg(long)]
                quiet: bool,
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
    fn skip_prefix_field_in_clap_struct_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            #[derive(clap::Args)]
            pub struct CheckCommand {
                #[arg(long)]
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
    fn no_prefix_field_in_parser_struct_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            #[derive(Parser)]
            pub struct Cli {
                #[arg(long)]
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
    fn explicit_negative_alias_or_long_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            #[derive(clap::Args)]
            pub struct CheckCommand {
                #[arg(long, alias = "skip-opinionated")]
                purist: bool,
            }
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/command.rs"), code);
        let rule = NoNegativeBoolRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(diag.message, contains_substring("skip-opinionated"));
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
    fn non_clap_struct_with_exact_skip_or_disabled_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
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
    fn affirmative_bool_fields_are_permitted() -> Result<(), Box<dyn std::error::Error>> {
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
