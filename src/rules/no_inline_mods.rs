//! Rule: `purist::no_inline_mods`
//!
//! # What This Rule Does
//! Forbids inline module definitions (`mod foo { ... }`) in crate entry point files (`main.rs` and `lib.rs`).
//! Dedicated unit test modules (`#[cfg(test)] mod tests { ... }`) and external module declarations
//! (`mod foo;`) are exempt.
//!
//! # Why This Rule Exists
//! Crate entry points (`lib.rs` and `main.rs`) should serve as concise indices of the crate's architecture,
//! public API exports, and top-level wiring. Defining inline modules in root files mixes high-level crate
//! declarations with implementation details, degrading readability and leading to monolithic file bloat.
//! Each logical subsystem belongs in its own dedicated file or directory.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! // In src/lib.rs:
//! mod helpers { // Inline module cluttering entry point
//!     pub fn format_text() { ... }
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! // In src/lib.rs:
//! pub mod helpers; // Declares file module
//!
//! // In src/helpers.rs:
//! pub fn format_text() { ... }
//! ```

use super::common::has_cfg_test_attr;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};

/// Rule enforcing modular project organization by forbidding inline modules in entry point files.
pub struct NoInlineModsRule;

impl Rule for NoInlineModsRule {
    fn name(&self) -> &'static str {
        "purist::no_inline_mods"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        // Only enforce in entry point files (main.rs, lib.rs, bin targets)
        if !ctx.is_main_or_lib() {
            return Vec::new();
        }

        let mut diagnostics = Vec::new();

        for item in &file.items {
            if let syn::Item::Mod(item_mod) = item
                && let Some(diag) = check_inline_module(ctx, self.name(), item_mod)
            {
                diagnostics.push(diag);
            }
        }

        diagnostics
    }
}

/// Checks whether an item module is an unidiomatic inline module in a crate root file.
fn check_inline_module(
    ctx: &LintContext<'_>,
    rule_name: &'static str,
    item_mod: &syn::ItemMod,
) -> Option<Diagnostic> {
    // Ignore file module declarations (e.g. `mod foo;` without inline body)
    item_mod.content.as_ref()?;

    // Ignore test modules (e.g. `#[cfg(test)] mod tests { ... }`)
    if has_cfg_test_attr(&item_mod.attrs) {
        return None;
    }

    let mod_name = item_mod.ident.to_string();
    let span = ctx.to_span(item_mod.ident.span());

    Some(
        Diagnostic::new(
            rule_name,
            Severity::Warning,
            format!(
                "Inline module '{mod_name}' in '{}' violates modularity guidelines. Submodules should be placed in dedicated files.",
                ctx.file_path().display()
            ),
        )
        .with_span(span)
        .with_suggested_fix(format!(
            "Move module content to '{mod_name}.rs' or '{mod_name}/mod.rs' and declare 'mod {mod_name};'."
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn inline_mod_in_main_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "mod helpers { pub fn run() {} }\n";
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInlineModsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::no_inline_mods"));
        assert_that!(&diag.message, contains_substring("Inline module 'helpers'"));
        assert_that!(
            diag.suggested_fix.as_deref(),
            eq(Some(
                "Move module content to 'helpers.rs' or 'helpers/mod.rs' and declare 'mod helpers;'."
            ))
        );
        Ok(())
    }

    #[googletest::test]
    fn cfg_test_inline_mod_in_lib_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn it_works() {}\n}\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInlineModsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn external_mod_declaration_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub mod cli;\npub mod tools;\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInlineModsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn inline_mod_in_submodule_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let source = "mod inner { pub fn helper() {} }\n";
        let ctx = LintContext::new(Path::new("src/utils/math.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInlineModsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
