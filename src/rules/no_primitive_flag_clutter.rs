//! # Rule: purist::no_primitive_flag_clutter
//!
//! ## What This Rule Does
//! Forbids accumulating loose boolean fields or generic flag tracking scopes (`FlagScope`) within
//! structs:
//! - In AST visitor structs: flags structs with 2 or more boolean or `FlagScope` fields.
//! - In general domain structs: flags structs with 3 or more loose boolean fields.
//!
//! ## Why This Rule Exists
//! Primitive Obsession with loose booleans (`in_main: bool`, `in_cli: bool`, `in_test: bool`,
//! `is_suppressed: bool`) leads to invalid state combinations, scattered state mutation, and naming
//! ambiguity for procedural derive macros. In visitor implementations, traversal flags should be
//! consolidated into cohesive domain scopes (e.g. `TestScope`, `DepthScope`, `ClapScope`, `MainScope`).
//! In domain structs, multiple flags should be encapsulated into an enum state machine or configuration struct.
//!
//! CLI argument models deriving `clap::Args` or `clap::Parser` are exempt.
//!
//! ## Non-Compliant Example
//! ```rust,ignore
//! struct MyVisitor {
//!     in_main_fn: bool,
//!     in_cli_runner: bool,
//!     is_suppressed: bool,
//! }
//! ```
//!
//! ## Compliant Example
//! ```rust,ignore
//! #[derive(WithTestScope, WithMainScope, WithClapScope)]
//! struct MyVisitor {
//!     test_scope: TestScope,
//!     main_scope: MainScope,
//!     clap_scope: ClapScope,
//! }
//! ```

use super::common::{
    TestScope, WithTestScope, derives_any, extract_type_ident, has_suppression_attribute,
    is_bool_type, path_ends_with_ident,
};
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use std::collections::HashSet;
use syn::visit::{self, Visit};
use syn::{Item, ItemImpl, ItemStruct, Type};

/// Rule forbidding excessive loose boolean and flag tracking fields in structs.
pub struct NoPrimitiveFlagClutterRule;

impl Rule for NoPrimitiveFlagClutterRule {
    fn name(&self) -> &'static str {
        "purist::no_primitive_flag_clutter"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let visitor_names = identify_visitor_struct_names(file);
        let mut visitor = StructFlagVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScope::new(ctx.is_test_file()),
            visitor_names,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Visitor inspecting struct definitions for primitive flag clutter.
#[derive(WithTestScope)]
struct StructFlagVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScope,
    visitor_names: HashSet<String>,
}

impl<'ast> Visit<'ast> for StructFlagVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });
    }

    fn visit_item_struct(&mut self, item_struct: &'ast ItemStruct) {
        if !self.test_scope.is_in_test()
            && !is_cli_clap_struct(item_struct)
            && !has_suppression_attribute(&item_struct.attrs, "no_primitive_flag_clutter")
        {
            self.check_struct_flag_clutter(item_struct);
        }

        visit::visit_item_struct(self, item_struct);
    }
}

impl StructFlagVisitor<'_> {
    fn check_struct_flag_clutter(&mut self, item_struct: &ItemStruct) {
        let struct_name = item_struct.ident.to_string();
        let is_visitor =
            self.visitor_names.contains(&struct_name) || struct_name.ends_with("Visitor");

        let flag_count = count_flag_fields(&item_struct.fields);

        if is_visitor && flag_count >= 2 {
            let span = self.ctx.to_span(item_struct.ident.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::no_primitive_flag_clutter",
                    Severity::Warning,
                    format!(
                        "Visitor struct '{struct_name}' contains multiple boolean/flag tracking fields ({flag_count}). Consolidate traversal state into cohesive domain scopes (e.g. 'TestScope', 'DepthScope', 'MainScope') instead of loose flags."
                    ),
                )
                .with_span(span)
                .with_suggested_fix("Consolidate loose flags into a domain scope with 'With...' traits and derive macros."),
            );
        } else if !is_visitor && flag_count >= 4 {
            let span = self.ctx.to_span(item_struct.ident.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "purist::no_primitive_flag_clutter",
                    Severity::Warning,
                    format!(
                        "Struct '{struct_name}' contains multiple loose boolean fields ({flag_count}). Encapsulate related boolean flags into a domain state enum, configuration struct, or bitflags."
                    ),
                )
                .with_span(span)
                .with_suggested_fix("Encapsulate boolean flags into an enum state machine or configuration type."),
            );
        }
    }
}

/// Identifies struct names that implement `Visit` or `VisitMut`.
fn identify_visitor_struct_names(file: &syn::File) -> HashSet<String> {
    let mut names = HashSet::new();
    for item in &file.items {
        if let Item::Impl(item_impl) = item
            && is_visit_trait_impl(item_impl)
            && let Some(ident) = extract_type_ident(&item_impl.self_ty)
        {
            names.insert(ident.to_string());
        }
    }
    names
}

/// Checks whether an implementation block implements `Visit` or `VisitMut`.
fn is_visit_trait_impl(item_impl: &ItemImpl) -> bool {
    item_impl.trait_.as_ref().is_some_and(|(path, _)| {
        path_ends_with_ident(path, "Visit") || path_ends_with_ident(path, "VisitMut")
    })
}

/// Counts fields in a struct that represent boolean or generic flag state.
fn count_flag_fields(fields: &syn::Fields) -> usize {
    fields.iter().filter(|f| is_flag_field_type(&f.ty)).count()
}

/// Returns true if a type represents a boolean or generic flag state.
fn is_flag_field_type(ty: &Type) -> bool {
    if is_bool_type(ty) {
        return true;
    }
    if let Type::Path(type_path) = ty {
        return path_ends_with_ident(&type_path.path, "FlagScope");
    }
    false
}

/// Returns true if a struct represents a CLI argument model.
fn is_cli_clap_struct(item_struct: &ItemStruct) -> bool {
    derives_any(&item_struct.attrs, &["Parser", "Args", "Subcommand"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn visitor_with_multiple_boolean_flags_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
struct SyntaxVisitor {
    in_main: bool,
    in_test: bool,
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrimitiveFlagClutterRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_primitive_flag_clutter"));
        assert_that!(diag.severity, eq(Severity::Warning));
        assert_that!(
            &diag.message,
            contains_substring(
                "Visitor struct 'SyntaxVisitor' contains multiple boolean/flag tracking fields (2)"
            )
        );
        Ok(())
    }

    #[googletest::test]
    fn domain_struct_with_four_booleans_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
struct UserAccount {
    is_active: bool,
    is_admin: bool,
    email_verified: bool,
    phone_verified: bool,
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrimitiveFlagClutterRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_primitive_flag_clutter"));
        assert_that!(diag.severity, eq(Severity::Warning));
        assert_that!(
            &diag.message,
            contains_substring("Struct 'UserAccount' contains multiple loose boolean fields (4)")
        );
        Ok(())
    }

    #[googletest::test]
    fn domain_struct_with_three_booleans_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
struct ConnectionOptions {
    keep_alive: bool,
    use_tls: bool,
    retry: bool,
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrimitiveFlagClutterRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn clap_args_struct_is_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(clap::Args)]
struct CliCommand {
    verbose: bool,
    quiet: bool,
    all: bool,
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrimitiveFlagClutterRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn suppressed_struct_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[expect(purist::no_primitive_flag_clutter, reason = "Legacy options struct")]
struct LegacyFlags {
    f1: bool,
    f2: bool,
    f3: bool,
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrimitiveFlagClutterRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
