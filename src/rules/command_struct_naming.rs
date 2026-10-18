//! Rule: `purist::command_struct_naming`
//!
//! # What This Rule Does
//! Enforces that Clap CLI command structs (structs deriving `clap::Args` or used as subcommand
//! payloads in `#[derive(clap::Subcommand)]` enums) follow the `<Subject>Command`, `<Subject>Args`,
//! or `<Subject>Subcommand` naming convention, while excluding reusable option bundles flattened via
//! `#[command(flatten)]` or `#[arg(flatten)]`.
//!
//! # Why This Rule Exists
//! In CLI applications built with Clap's derive API, consistent naming of executable command
//! models makes the CLI architecture self-documenting and predictable. Unadorned names like bare
//! `Subcommand` (without a `<Subject>` prefix) or arbitrary suffixes like `*Params`, `*Payload`,
//! `*Options`, or generic nouns obscure which structs represent executable CLI commands versus
//! reusable flattened argument groups.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! #[derive(clap::Args)]
//! pub struct Subcommand { // Bare 'Subcommand' lacks a <Subject> prefix
//!     force: bool,
//! }
//!
//! #[derive(clap::Args)]
//! pub struct BuildParams { // Non-standard 'Params' suffix for a command struct
//!     release: bool,
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! #[derive(clap::Args)]
//! pub struct WorkspaceSubcommand {
//!     force: bool,
//! }
//!
//! #[derive(clap::Args)]
//! pub struct BuildCommand {
//!     #[command(flatten)]
//!     common: CommonOptions, // Permitted when flattened as a reusable option bundle
//!     release: bool,
//! }
//!
//! #[derive(clap::Args)]
//! pub struct CommonOptions {
//!     verbose: bool,
//! }
//! ```

use super::common::{
    TestScope, WithTestScope, derives_trait, extract_type_ident, has_suppression_attribute,
};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use crate::scopes::{
    TypeScope, WithTypeScope, is_command_execution_fn_name, is_command_struct_name,
    is_flattened_field,
};
use std::collections::HashSet;
use syn::visit::{self, Visit};

/// Rule enforcing `<Subject>Command`, `<Subject>Args`, or `<Subject>Subcommand` naming for Clap command structs.
pub(crate) struct CommandStructNamingRule;

impl Rule for CommandStructNamingRule {
    fn name(&self) -> &'static str {
        "purist::command_struct_naming"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut collector = ClapCommandUsageCollector::default();
        collector.visit_file(file);

        let mut visitor = CommandStructNamingVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScope::new(ctx.is_test_file()),
            flattened_types: collector.flattened_types,
            subcommand_payload_types: collector.subcommand_payload_types,
            executable_types: collector.executable_types,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// First-pass AST collector identifying flattened option structs, subcommand variant payloads,
/// and structs with CLI command execution methods (`run`, `execute`).
#[derive(Default, WithTypeScope)]
struct ClapCommandUsageCollector {
    type_scope: TypeScope,
    flattened_types: HashSet<String>,
    subcommand_payload_types: HashSet<String>,
    executable_types: HashSet<String>,
}

impl<'ast> Visit<'ast> for ClapCommandUsageCollector {
    fn visit_item_enum(&mut self, item_enum: &'ast syn::ItemEnum) {
        if derives_trait(&item_enum.attrs, "Subcommand") {
            self.with_type_enum(&item_enum.ident, |this| {
                visit::visit_item_enum(this, item_enum);
            });
        } else {
            visit::visit_item_enum(self, item_enum);
        }
    }

    fn visit_field(&mut self, field: &'ast syn::Field) {
        if self.type_scope.current_enum_name().is_some() {
            if field.ident.is_none()
                && let Some(ident) = extract_type_ident(&field.ty)
            {
                self.subcommand_payload_types.insert(ident.to_string());
            }
        } else if is_flattened_field(field)
            && let Some(ident) = extract_type_ident(&field.ty)
        {
            self.flattened_types.insert(ident.to_string());
        }

        visit::visit_field(self, field);
    }

    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        if let Some(ident) = extract_type_ident(&item_impl.self_ty) {
            self.with_type_impl(ident.to_string(), |this| {
                visit::visit_item_impl(this, item_impl);
            });
        } else {
            visit::visit_item_impl(self, item_impl);
        }
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        if let Some(struct_name) = self.type_scope.current_impl_name() {
            let fn_name = impl_fn.sig.ident.to_string();
            if is_command_execution_fn_name(&fn_name) {
                self.executable_types.insert(struct_name.to_string());
            }
        }
        visit::visit_impl_item_fn(self, impl_fn);
    }
}

/// Visitor that validates Clap command struct identifiers against `<Subject>Command`,
/// `<Subject>Args`, and `<Subject>Subcommand` conventions.
#[derive(WithTestScope)]
struct CommandStructNamingVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScope,
    flattened_types: HashSet<String>,
    subcommand_payload_types: HashSet<String>,
    executable_types: HashSet<String>,
}

impl<'ast> Visit<'ast> for CommandStructNamingVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });
    }

    fn visit_item_struct(&mut self, item_struct: &'ast syn::ItemStruct) {
        if !self.test_scope.is_in_test()
            && !has_suppression_attribute(&item_struct.attrs, "command_struct_naming")
        {
            self.check_clap_struct_command_naming(item_struct);
        }

        visit::visit_item_struct(self, item_struct);
    }
}

impl CommandStructNamingVisitor<'_> {
    fn check_clap_struct_command_naming(&mut self, item_struct: &syn::ItemStruct) {
        let struct_name = item_struct.ident.to_string();
        let derives_args = derives_trait(&item_struct.attrs, "Args");
        let is_subcommand_payload = self.subcommand_payload_types.contains(&struct_name);

        if !derives_args && !is_subcommand_payload {
            return;
        }

        // Exclude reusable option bundles flattened via #[command(flatten)] / #[arg(flatten)]
        // unless they are also used directly as subcommand payloads or define run/execute methods.
        let is_only_flattened = self.flattened_types.contains(&struct_name)
            && !is_subcommand_payload
            && !self.executable_types.contains(&struct_name);
        if is_only_flattened {
            return;
        }

        if is_command_struct_name(&struct_name) {
            return;
        }

        let span = self.ctx.to_span(item_struct.ident.span());
        let is_bare_suffix = matches!(
            struct_name.as_str(),
            "Command" | "Args" | "Subcommand" | "Subcommands"
        );

        let (message, suggested_fix) = if is_bare_suffix {
            (
                format!(
                    "Clap command struct '{struct_name}' uses an unadorned suffix without a subject prefix. Follow the '<Subject>Command', '<Subject>Args', or '<Subject>Subcommand' naming convention."
                ),
                format!(
                    "Prefix '{struct_name}' with a descriptive subject (e.g. 'Workspace{struct_name}' or 'BuildCommand')."
                ),
            )
        } else {
            (
                format!(
                    "Clap command struct '{struct_name}' does not follow the CLI command naming convention. Command structs must be named '<Subject>Command', '<Subject>Args', or '<Subject>Subcommand'."
                ),
                "Rename the struct to end with 'Command', 'Args', or 'Subcommand' (e.g. '<Subject>Command', '<Subject>Args', or '<Subject>Subcommand').".to_string(),
            )
        };

        self.diagnostics.push(
            Diagnostic::new("purist::command_struct_naming", Severity::Warning, message)
                .with_span(span)
                .with_suggested_fix(suggested_fix),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn valid_command_args_and_subcommand_names_are_permitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(clap::Args)]
pub struct BuildCommand {
    release: bool,
}

#[derive(clap::Args)]
pub struct ConfigInitArgs {
    global: bool,
}

#[derive(clap::Args)]
pub struct WorkspaceSubcommand {
    force: bool,
}
"#;
        let ctx = LintContext::new(Path::new("src/commands.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CommandStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn bare_subcommand_struct_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(clap::Args)]
pub struct Subcommand {
    force: bool,
}
"#;
        let ctx = LintContext::new(Path::new("src/commands.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CommandStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::command_struct_naming"));
        assert_that!(
            &diag.message,
            contains_substring("uses an unadorned suffix without a subject prefix")
        );
        Ok(())
    }

    #[googletest::test]
    fn non_standard_command_struct_suffix_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(clap::Args)]
pub struct BuildParams {
    release: bool,
}

#[derive(clap::Args)]
pub struct DeployPayload {
    env: String,
}
"#;
        let ctx = LintContext::new(Path::new("src/commands.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CommandStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(2));
        let first = diags.first().ok_or("expected first diagnostic")?;
        assert_that!(&first.rule, eq("purist::command_struct_naming"));
        assert_that!(
            &first.message,
            contains_substring("Clap command struct 'BuildParams' does not follow")
        );
        Ok(())
    }

    #[googletest::test]
    fn flattened_option_bundle_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(clap::Args)]
pub struct RunCommand {
    #[command(flatten)]
    common: CommonOptions,
    #[arg(flatten)]
    filter: FilterFlags,
}

#[derive(clap::Args)]
pub struct CommonOptions {
    verbose: bool,
}

#[derive(clap::Args)]
pub struct FilterFlags {
    include: Option<String>,
}
"#;
        let ctx = LintContext::new(Path::new("src/commands.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CommandStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn subcommand_enum_payload_with_invalid_name_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(clap::Subcommand)]
pub enum Commands {
    Sync(SyncOptions),
}

pub struct SyncOptions {
    dry_run: bool,
}
"#;
        let ctx = LintContext::new(Path::new("src/commands.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CommandStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            &diag.message,
            contains_substring("Clap command struct 'SyncOptions'")
        );
        Ok(())
    }

    #[googletest::test]
    fn suppressed_command_struct_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[expect(purist::command_struct_naming, reason = "Legacy CLI struct name")]
#[derive(clap::Args)]
pub struct LegacyParams {
    verbose: bool,
}
"#;
        let ctx = LintContext::new(Path::new("src/commands.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CommandStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
