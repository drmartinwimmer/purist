use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule forbidding `unsafe` blocks and functions in test suites.
pub struct NoUnsafeInTestsRule;

impl Rule for NoUnsafeInTestsRule {
    fn name(&self) -> &'static str {
        "opinionated::no_unsafe_in_tests"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = UnsafeTestVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_cfg_test: ctx.is_test_file(),
            current_fn_is_test: false,
            suppressed_depth: 0,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

struct UnsafeTestVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_cfg_test: bool,
    current_fn_is_test: bool,
    suppressed_depth: usize,
}

impl UnsafeTestVisitor<'_> {
    fn has_suppression(&self, attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            if !attr.path().is_ident("expect") && !attr.path().is_ident("allow") {
                return false;
            }
            let mut matched_rule = false;
            let _result = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("no_unsafe_in_tests")
                    || meta
                        .path
                        .segments
                        .last()
                        .map(|s| s.ident == "no_unsafe_in_tests")
                        .unwrap_or(false)
                {
                    matched_rule = true;
                }
                Ok(())
            });
            matched_rule
        })
    }
}

impl<'ast> Visit<'ast> for UnsafeTestVisitor<'_> {
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

        let suppressed = self.has_suppression(&item_mod.attrs);
        if suppressed {
            self.suppressed_depth += 1;
        }

        let prev = self.in_cfg_test;
        if is_cfg_test {
            self.in_cfg_test = true;
        }

        visit::visit_item_mod(self, item_mod);
        self.in_cfg_test = prev;

        if suppressed {
            self.suppressed_depth -= 1;
        }
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let is_test_attr = item_fn.attrs.iter().any(|attr| {
            attr.path().is_ident("test")
                || attr
                    .path()
                    .segments
                    .last()
                    .map(|s| s.ident == "test")
                    .unwrap_or(false)
        });

        let suppressed = self.has_suppression(&item_fn.attrs);
        if suppressed {
            self.suppressed_depth += 1;
        }

        let prev = self.current_fn_is_test;
        if is_test_attr {
            self.current_fn_is_test = true;
        }

        if (self.in_cfg_test || self.current_fn_is_test)
            && item_fn.sig.unsafety.is_some()
            && self.suppressed_depth == 0
        {
            let span = self.ctx.to_span(item_fn.sig.fn_token.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "opinionated::no_unsafe_in_tests",
                    Severity::Error,
                    format!(
                        "Function '{}' in test context is declared 'unsafe'. Tests must verify code through safe interfaces.",
                        item_fn.sig.ident
                    ),
                )
                .with_span(span)
                .with_suggested_fix("Remove 'unsafe' from test code; verify behavior strictly through safe public interfaces."),
            );
        }

        visit::visit_item_fn(self, item_fn);
        self.current_fn_is_test = prev;

        if suppressed {
            self.suppressed_depth -= 1;
        }
    }

    fn visit_expr_unsafe(&mut self, expr_unsafe: &'ast syn::ExprUnsafe) {
        if (self.in_cfg_test || self.current_fn_is_test) && self.suppressed_depth == 0 {
            let span = self.ctx.to_span(expr_unsafe.unsafe_token.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "opinionated::no_unsafe_in_tests",
                    Severity::Error,
                    "Usage of 'unsafe' block in test context. Tests must exercise safe public abstractions.",
                )
                .with_span(span)
                .with_suggested_fix("Remove 'unsafe' block from test code; verify behavior strictly through safe public interfaces."),
            );
        }

        visit::visit_expr_unsafe(self, expr_unsafe);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn unsafe_block_in_test_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn test_foo() {
    let x = 42;
    unsafe {
        let ptr = &x as *const i32;
        let _val = *ptr;
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoUnsafeInTestsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::no_unsafe_in_tests"));
        assert_that!(diag.severity, eq(Severity::Error));
        Ok(())
    }

    #[googletest::test]
    fn suppressed_unsafe_in_test_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
#[expect(opinionated::no_unsafe_in_tests, reason = "FFI test requirement")]
fn test_ffi_boundary() {
    unsafe {
        let _ = libc::getpid();
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/tests.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoUnsafeInTestsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn unsafe_in_production_code_is_ignored_by_this_rule() -> Result<(), Box<dyn std::error::Error>>
    {
        let source = r#"
pub fn safe_wrapper() {
    unsafe {
        std::hint::unreachable_unchecked();
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/core.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoUnsafeInTestsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
