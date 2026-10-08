//! Rule: `purist::clap_struct_encapsulation`
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

use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use crate::scopes::{
    ClapScope, TestScope, WithClapScope, WithTestScope, is_command_execution_fn_name,
    is_command_struct_name, is_flattened_field,
};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule enforcing encapsulation for Clap CLI models (private fields and associated `run` method).
pub struct ClapEncapsulationRule;

impl Rule for ClapEncapsulationRule {
    fn name(&self) -> &'static str {
        "purist::clap_struct_encapsulation"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut collector = RunMethodCollector::default();
        collector.visit_file(file);

        let mut visitor = ClapEncapsulationVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScope::new(ctx.is_test_file()),
            clap_scope: ClapScope::new(),
            struct_has_run_method: collector.struct_has_run_method,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor that collects all struct names that define an associated `run` or `execute` method.
#[derive(Default)]
struct RunMethodCollector {
    struct_has_run_method: HashSet<String>,
}

impl<'ast> Visit<'ast> for RunMethodCollector {
    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        if let syn::Type::Path(type_path) = &*item_impl.self_ty
            && let Some(ident) = type_path.path.get_ident()
        {
            for impl_item in &item_impl.items {
                if let syn::ImplItem::Fn(impl_fn) = impl_item {
                    let method_name = impl_fn.sig.ident.to_string();
                    if is_command_execution_fn_name(&method_name) {
                        self.struct_has_run_method.insert(ident.to_string());
                    }
                }
            }
        }
        visit::visit_item_impl(self, item_impl);
    }
}

/// Visitor that inspects Clap structs and their fields using RAII scope tracking.
#[derive(WithTestScope, WithClapScope)]
struct ClapEncapsulationVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScope,
    clap_scope: ClapScope,
    struct_has_run_method: HashSet<String>,
}

impl<'ast> Visit<'ast> for ClapEncapsulationVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });
    }

    fn visit_item_struct(&mut self, item_struct: &'ast syn::ItemStruct) {
        self.with_clap_struct(item_struct, |this| {
            if !this.test_scope.is_in_test() && this.clap_scope.is_in_clap_struct() {
                this.check_clap_struct_missing_run_method(item_struct);
            }

            visit::visit_item_struct(this, item_struct);
        });
    }

    fn visit_field(&mut self, field: &'ast syn::Field) {
        if !self.test_scope.is_in_test() && self.clap_scope.is_in_clap_struct() {
            self.check_clap_field_public_visibility(field);
        }

        visit::visit_field(self, field);
    }
}

impl ClapEncapsulationVisitor<'_> {
    fn check_clap_struct_missing_run_method(&mut self, item_struct: &syn::ItemStruct) {
        let struct_name = item_struct.ident.to_string();
        if is_command_struct_name(&struct_name)
            && !self.struct_has_run_method.contains(&struct_name)
        {
            let span = self.ctx.to_span(item_struct.ident.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::clap_struct_encapsulation",
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

    fn check_clap_field_public_visibility(&mut self, field: &syn::Field) {
        if matches!(field.vis, syn::Visibility::Public(_)) && !is_flattened_field(field) {
            let struct_name = self.clap_scope.current_struct_name().unwrap_or("unknown");
            let field_name = field
                .ident
                .as_ref()
                .map(|i| i.to_string())
                .unwrap_or_else(|| "unnamed".to_string());
            let span = self.ctx.to_span(field.vis.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::clap_struct_encapsulation",
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
        assert_that!(&diag.rule, eq("purist::clap_struct_encapsulation"));
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
        assert_that!(&diag.rule, eq("purist::clap_struct_encapsulation"));
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
