use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging wildcard `*` imports outside test contexts and preludes.
pub struct NoWildcardImportsRule;

impl Rule for NoWildcardImportsRule {
    fn name(&self) -> &'static str {
        "opinionated::no_wildcard_imports"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = WildcardImportVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_test_scope: ctx.is_test_file(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

struct WildcardImportVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_test_scope: bool,
}

impl<'ast> Visit<'ast> for WildcardImportVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let is_cfg_test = item_mod.attrs.iter().any(|attr| {
            if !attr.path().is_ident("cfg") {
                return false;
            }
            let mut test_attr = false;
            let _result = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("test") {
                    test_attr = true;
                }
                Ok(())
            });
            test_attr
        });

        let prev = self.in_test_scope;
        if is_cfg_test {
            self.in_test_scope = true;
        }

        visit::visit_item_mod(self, item_mod);
        self.in_test_scope = prev;
    }

    fn visit_item_use(&mut self, item_use: &'ast syn::ItemUse) {
        if !self.in_test_scope {
            self.check_use_tree(&item_use.tree, Vec::new());
        }

        visit::visit_item_use(self, item_use);
    }
}

impl WildcardImportVisitor<'_> {
    fn check_use_tree(&mut self, tree: &syn::UseTree, mut path_segments: Vec<String>) {
        match tree {
            syn::UseTree::Path(p) => {
                path_segments.push(p.ident.to_string());
                self.check_use_tree(&p.tree, path_segments);
            }
            syn::UseTree::Glob(g) => {
                // Exempt preludes like googletest::prelude::* or std::io::prelude::*
                let is_prelude = path_segments
                    .last()
                    .map(|s| s == "prelude")
                    .unwrap_or(false);
                if !is_prelude {
                    let path_str = if path_segments.is_empty() {
                        "*".to_string()
                    } else {
                        format!("{}::*", path_segments.join("::"))
                    };
                    let span = self.ctx.to_span(g.span());
                    self.diagnostics.push(
                        Diagnostic::new(
                            "opinionated::no_wildcard_imports",
                            Severity::Warning,
                            format!(
                                "Avoid wildcard import '{path_str}'. Wildcard imports obscure symbol provenance and cause namespace pollution."
                            ),
                        )
                        .with_span(span)
                        .with_suggested_fix("Replace wildcard import with explicit item imports."),
                    );
                }
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.check_use_tree(item, path_segments.clone());
                }
            }
            syn::UseTree::Name(_) | syn::UseTree::Rename(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn wildcard_import_in_library_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "use std::collections::*;\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoWildcardImportsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::no_wildcard_imports"));
        assert_that!(&diag.message, contains_substring("std::collections::*"));
        Ok(())
    }

    #[googletest::test]
    fn prelude_wildcard_import_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "use googletest::prelude::*;\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoWildcardImportsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn wildcard_in_test_module_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[cfg(test)]
mod tests {
    use super::*;
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoWildcardImportsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
