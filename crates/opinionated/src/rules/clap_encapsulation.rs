//! Rule: `opinionated::clap_struct_encapsulation`
//!
//! # What This Rule Does
//! Enforces encapsulation standards on CLI command structs that derive Clap traits (`clap::Args`, `clap::Parser`):
//! 1. Command argument fields must remain private (unless marked with `#[command(flatten)]` or `#[arg(flatten)]`).
//! 2. Command structs (names ending in `Command` or `Args`, but not shared options/flags) must define an associated `run` or `execute` method.
//!
//! # Why This Rule Exists
//! Exposing raw CLI arguments as public struct fields encourages external modules to reach into command
//! representations, breaking encapsulation and coupling caller code to CLI argument shapes. Furthermore,
//! requiring an associated `run` or `execute` method ensures commands are self-contained executable units
//! rather than passive data containers whose execution logic is scattered across large match statements.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! #[derive(clap::Args)]
//! pub struct BuildCommand {
//!     pub release: bool, // Public argument field leaks CLI representation
//! }
//! // Lacks an associated run/execute method
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! #[derive(clap::Args)]
//! pub struct BuildCommand {
//!     release: bool, // Encapsulated private field
//! }
//!
//! impl BuildCommand {
//!     pub fn run(self) -> Result<(), BuildError> {
//!         // Execution logic encapsulated within the command
//!         Ok(())
//!     }
//! }
//! ```

use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;

/// Rule enforcing encapsulation for Clap CLI models (private fields and associated `run` method).
pub struct ClapEncapsulationRule;

impl Rule for ClapEncapsulationRule {
    fn name(&self) -> &'static str {
        "opinionated::clap_struct_encapsulation"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let (clap_structs, struct_has_run_method) = collect_clap_structs_and_methods(file);
        let mut diagnostics = Vec::new();

        for (struct_name, item_struct) in &clap_structs {
            // Check 1: Struct fields must remain private unless marked with flatten
            check_field_visibility(ctx, self.name(), struct_name, item_struct, &mut diagnostics);

            // Check 2: Command structs should have an associated run or execute method
            check_command_execution_method(
                ctx,
                self.name(),
                struct_name,
                item_struct,
                &struct_has_run_method,
                &mut diagnostics,
            );
        }

        diagnostics
    }
}

/// Collects all structs that derive Clap traits and identifies structs with associated `run`/`execute` methods.
fn collect_clap_structs_and_methods(
    file: &syn::File,
) -> (HashMap<String, &syn::ItemStruct>, HashSet<String>) {
    let mut clap_structs = HashMap::new();
    let mut struct_has_run_method = HashSet::new();

    for item in &file.items {
        match item {
            syn::Item::Struct(item_struct) => {
                if derives_clap(&item_struct.attrs) {
                    clap_structs.insert(item_struct.ident.to_string(), item_struct);
                }
            }
            syn::Item::Impl(item_impl) => {
                if let syn::Type::Path(type_path) = &*item_impl.self_ty
                    && let Some(ident) = type_path.path.get_ident()
                {
                    for impl_item in &item_impl.items {
                        if let syn::ImplItem::Fn(impl_fn) = impl_item {
                            let method_name = impl_fn.sig.ident.to_string();
                            if method_name == "run" || method_name == "execute" {
                                struct_has_run_method.insert(ident.to_string());
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    (clap_structs, struct_has_run_method)
}

/// Checks that all fields of a Clap struct are private unless explicitly flattened.
fn check_field_visibility(
    ctx: &LintContext<'_>,
    rule_name: &'static str,
    struct_name: &str,
    item_struct: &syn::ItemStruct,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for field in &item_struct.fields {
        if matches!(field.vis, syn::Visibility::Public(_)) && !is_flattened_field(field) {
            let field_name = field
                .ident
                .as_ref()
                .map(|i| i.to_string())
                .unwrap_or_else(|| "unnamed".to_string());
            let span = ctx.to_span(field.vis.span());
            diagnostics.push(
                Diagnostic::new(
                    rule_name,
                    Severity::Warning,
                    format!(
                        "Field '{field_name}' in Clap struct '{struct_name}' is declared public. Clap argument fields should remain private to encapsulate command execution."
                    ),
                )
                .with_span(span)
                .with_suggested_fix("Make field private and encapsulate logic within struct methods."),
            );
        }
    }
}

/// Checks that command structs define an associated `run` or `execute` method.
fn check_command_execution_method(
    ctx: &LintContext<'_>,
    rule_name: &'static str,
    struct_name: &str,
    item_struct: &syn::ItemStruct,
    struct_has_run_method: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if is_command_struct_name(struct_name) && !struct_has_run_method.contains(struct_name) {
        let span = ctx.to_span(item_struct.ident.span());
        diagnostics.push(
            Diagnostic::new(
                rule_name,
                Severity::Warning,
                format!(
                    "Clap command struct '{struct_name}' lacks an associated 'run' or 'execute' method. Encapsulate execution inside an associated method."
                ),
            )
            .with_span(span)
            .with_suggested_fix("Implement 'pub fn run(&self, ...)' for this command struct."),
        );
    }
}

/// Checks whether an attribute list includes a derive for `Args` or `Parser`.
fn derives_clap(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let mut found_clap = false;
        let _result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("Args")
                || meta.path.is_ident("Parser")
                || meta
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident == "Args" || s.ident == "Parser")
                    .unwrap_or(false)
            {
                found_clap = true;
            }
            Ok(())
        });
        found_clap
    })
}

/// Checks whether a struct field is annotated with `#[command(flatten)]` or `#[arg(flatten)]`.
fn is_flattened_field(field: &syn::Field) -> bool {
    field.attrs.iter().any(|attr| {
        if !attr.path().is_ident("command") && !attr.path().is_ident("arg") {
            return false;
        }
        let mut is_flatten = false;
        let _result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("flatten") {
                is_flatten = true;
            }
            Ok(())
        });
        is_flatten
    })
}

/// Returns true if the struct represents an executable command rather than shared configuration or options.
fn is_command_struct_name(name: &str) -> bool {
    // Shared options/config/flags or root CLI parser are not single commands
    if name.ends_with("Options")
        || name.ends_with("Opts")
        || name.ends_with("Config")
        || name.ends_with("Flags")
        || name.starts_with("Common")
        || name == "Cli"
        || name == "Args"
    {
        return false;
    }

    // Typical command structs end in Command or represent a specific action
    name.ends_with("Command") || name.ends_with("Args")
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn public_clap_field_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(clap::Args)]
pub struct LintCommand {
    pub fix: bool,
}

impl LintCommand {
    pub fn run(&self) {}
}
"#;
        let ctx = LintContext::new(Path::new("src/commands/lint.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ClapEncapsulationRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::clap_struct_encapsulation"));
        assert_that!(
            &diag.message,
            contains_substring("Field 'fix' in Clap struct 'LintCommand' is declared public")
        );
        Ok(())
    }

    #[googletest::test]
    fn command_struct_without_run_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(clap::Args)]
pub struct CheckCommand {
    path: String,
}
"#;
        let ctx = LintContext::new(Path::new("src/commands/check.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ClapEncapsulationRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::clap_struct_encapsulation"));
        assert_that!(
            &diag.message,
            contains_substring("lacks an associated 'run' or 'execute' method")
        );
        Ok(())
    }

    #[googletest::test]
    fn encapsulated_command_with_run_method_is_permitted() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = r#"
#[derive(clap::Args)]
pub struct FormatCommand {
    check_only: bool,
}

impl FormatCommand {
    pub fn run(&self) -> Result<(), String> {
        Ok(())
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/commands/format.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ClapEncapsulationRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn flattened_field_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(clap::Args)]
pub struct RunCommand {
    #[command(flatten)]
    pub options: CommonOptions,
}

impl RunCommand {
    pub fn run(&self) {}
}
"#;
        let ctx = LintContext::new(Path::new("src/commands/run.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ClapEncapsulationRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
