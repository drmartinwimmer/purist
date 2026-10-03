use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};

/// Rule enforcing modular project organization by forbidding inline modules in entry point files.
pub struct NoInlineModsRule;

impl Rule for NoInlineModsRule {
    fn name(&self) -> &'static str {
        "opinionated::no_inline_mods"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Only enforce in entry point files (main.rs, lib.rs, bin targets)
        if !ctx.is_main_or_lib() {
            return diagnostics;
        }

        for item in &file.items {
            if let syn::Item::Mod(item_mod) = item {
                // Ignore file module declarations (e.g. `mod foo;` without inline body)
                if item_mod.content.is_none() {
                    continue;
                }

                // Ignore test modules (e.g. `#[cfg(test)] mod tests { ... }`)
                if is_cfg_test_module(item_mod) {
                    continue;
                }

                let mod_name = item_mod.ident.to_string();
                let span = ctx.to_span(item_mod.ident.span());

                diagnostics.push(
                    Diagnostic::new(
                        self.name(),
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
                );
            }
        }

        diagnostics
    }
}

/// Checks whether a module is annotated with `#[cfg(test)]`.
fn is_cfg_test_module(item_mod: &syn::ItemMod) -> bool {
    item_mod.attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }

        let mut is_test = false;
        let _result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("test") {
                is_test = true;
            }
            Ok(())
        });
        is_test
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn test_inline_mod_in_main_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "mod helpers { pub fn run() {} }\n";
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInlineModsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        assert_that!(diags[0].rule.as_str(), eq("opinionated::no_inline_mods"));
        assert_that!(
            diags[0].message,
            contains_substring("Inline module 'helpers'")
        );
        assert_that!(
            diags[0].suggested_fix.as_deref(),
            eq(Some(
                "Move module content to 'helpers.rs' or 'helpers/mod.rs' and declare 'mod helpers;'."
            ))
        );
        Ok(())
    }

    #[googletest::test]
    fn test_cfg_test_inline_mod_in_lib_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "#[cfg(test)]\nmod tests {\n    #[test]\n    fn it_works() {}\n}\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInlineModsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn test_external_mod_declaration_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub mod cli;\npub mod tools;\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInlineModsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn test_inline_mod_in_submodule_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let source = "mod inner { pub fn helper() {} }\n";
        let ctx = LintContext::new(Path::new("src/utils/math.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoInlineModsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
