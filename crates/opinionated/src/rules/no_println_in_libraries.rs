use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging `println!` / `eprintln!` in library modules.
pub struct NoPrintlnInLibrariesRule;

impl Rule for NoPrintlnInLibrariesRule {
    fn name(&self) -> &'static str {
        "opinionated::no_println_in_libraries"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if is_exempt_entrypoint_or_example(ctx) {
            return Vec::new();
        }

        let mut visitor = PrintlnVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_test_scope: ctx.is_test_file(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

fn is_exempt_entrypoint_or_example(ctx: &LintContext<'_>) -> bool {
    let path_str = ctx.file_path().to_string_lossy();
    path_str.ends_with("main.rs") || path_str.contains("/bin/") || path_str.contains("/examples/")
}

struct PrintlnVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_test_scope: bool,
}

impl<'ast> Visit<'ast> for PrintlnVisitor<'_> {
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

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let is_test = item_fn.attrs.iter().any(|attr| {
            attr.path().is_ident("test")
                || attr
                    .path()
                    .segments
                    .last()
                    .map(|s| s.ident == "test")
                    .unwrap_or(false)
        });

        let prev = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }

        visit::visit_item_fn(self, item_fn);
        self.in_test_scope = prev;
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if !self.in_test_scope
            && let Some(segment) = mac.path.segments.last()
        {
            let name = segment.ident.to_string();
            if name == "println" || name == "eprintln" {
                let span = self.ctx.to_span(mac.path.span());
                self.diagnostics.push(
                    Diagnostic::new(
                        "opinionated::no_println_in_libraries",
                        Severity::Warning,
                        format!(
                            "Direct use of '{name}!' in library code. Library code should return structured errors or use logging."
                        ),
                    )
                    .with_span(span)
                    .with_suggested_fix("Remove print statement; propagate information via 'Result' or use 'tracing'/'log'."),
                );
            }
        }

        visit::visit_macro(self, mac);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn println_in_library_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn parse_input() {
    println!("Parsing started...");
}
"#;
        let ctx = LintContext::new(Path::new("src/parser.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrintlnInLibrariesRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::no_println_in_libraries"));
        assert_that!(&diag.message, contains_substring("println!"));
        Ok(())
    }

    #[googletest::test]
    fn println_in_main_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
fn main() {
    println!("Hello, CLI!");
}
"#;
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrintlnInLibrariesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn println_in_test_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn runs_test() {
    println!("debug test info");
}
"#;
        let ctx = LintContext::new(Path::new("src/parser.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoPrintlnInLibrariesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
