use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule enforcing suppression hygiene: requiring reason="…" and a preceding explanatory comment.
pub struct ClippySuppressRule;

impl Rule for ClippySuppressRule {
    fn name(&self) -> &'static str {
        "opinionated::clippy_suppression_hygiene"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = SuppressVisitor {
            ctx,
            diagnostics: Vec::new(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

struct SuppressVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for SuppressVisitor<'_> {
    fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
        let is_suppression = attr.path().is_ident("allow") || attr.path().is_ident("expect");

        if is_suppression {
            let mut has_reason = false;
            let _result = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("reason") {
                    has_reason = true;
                }
                Ok(())
            });

            let span = self.ctx.to_span(attr.span());
            let start_line = attr.span().start().line;
            let has_comment = self.ctx.has_preceding_comment(start_line);

            if !has_reason && !has_comment {
                self.diagnostics.push(
                    Diagnostic::new(
                        "opinionated::clippy_suppression_hygiene",
                        Severity::Warning,
                        "Lint suppression attribute lacks both a 'reason = \"...\"' parameter and an explanatory code comment on the preceding line.",
                    )
                    .with_span(span)
                    .with_suggested_fix("Add reason = \"...\" to the attribute and document the rationale in a comment directly above it."),
                );
            } else if !has_reason {
                self.diagnostics.push(
                    Diagnostic::new(
                        "opinionated::clippy_suppression_hygiene",
                        Severity::Warning,
                        "Lint suppression attribute lacks a 'reason = \"...\"' parameter explaining why it is necessary.",
                    )
                    .with_span(span)
                    .with_suggested_fix("Add reason = \"...\" parameter to the suppression attribute."),
                );
            } else if !has_comment {
                self.diagnostics.push(
                    Diagnostic::new(
                        "opinionated::clippy_suppression_hygiene",
                        Severity::Warning,
                        "Lint suppression attribute lacks an accompanying code comment on the preceding line explaining why the lint cannot be resolved.",
                    )
                    .with_span(span)
                    .with_suggested_fix("Add an explanatory '// ...' comment on the line immediately preceding the attribute."),
                );
            }
        }

        visit::visit_attribute(self, attr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn unhygienic_allow_without_reason_or_comment_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = "#[allow(clippy::unwrap_used)]\nfn foo() {}\n";
        let ctx = LintContext::new(Path::new("src/foo.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ClippySuppressRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::clippy_suppression_hygiene"));
        assert_that!(&diag.message, contains_substring("lacks both a 'reason"));
        Ok(())
    }

    #[googletest::test]
    fn allow_with_reason_but_no_comment_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "#[allow(clippy::unwrap_used, reason = \"safe\")]\nfn foo() {}\n";
        let ctx = LintContext::new(Path::new("src/foo.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ClippySuppressRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::clippy_suppression_hygiene"));
        assert_that!(
            &diag.message,
            contains_substring("lacks an accompanying code comment")
        );
        Ok(())
    }

    #[googletest::test]
    fn allow_with_comment_but_no_reason_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "// Reason explained here\n#[allow(clippy::unwrap_used)]\nfn foo() {}\n";
        let ctx = LintContext::new(Path::new("src/foo.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ClippySuppressRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::clippy_suppression_hygiene"));
        assert_that!(
            &diag.message,
            contains_substring("lacks a 'reason = \"...\"' parameter")
        );
        Ok(())
    }

    #[googletest::test]
    fn hygienic_expect_with_both_comment_and_reason_is_permitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = "// Tested invariant guarantees no panic here\n#[expect(clippy::unwrap_used, reason = \"infallible after check\")]\nfn foo() {}\n";
        let ctx = LintContext::new(Path::new("src/foo.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ClippySuppressRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
