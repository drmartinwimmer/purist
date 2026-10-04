//! Rule: `opinionated::clippy_suppression_hygiene`
//!
//! # What This Rule Does
//! Enforces rigorous hygiene on compiler and Clippy lint suppressions (`#[allow(...)]` and `#[expect(...)]`).
//! Specifically, every lint suppression attribute must:
//! 1. Include an in-attribute `reason = "..."` parameter documenting why the lint is waived.
//! 2. Have an accompanying code comment (`// ...` or `/* ... */`) on the immediately preceding line
//!    explaining why the code cannot be refactored to resolve the lint cleanly.
//!
//! # Why This Rule Exists
//! Silent or unmotivated lint suppressions quickly accumulate technical debt and mask bugs. Requiring
//! both a machine-readable reason parameter and a human-readable explanatory comment ensures that
//! every suppression is an intentional, documented engineering decision rather than a quick workaround.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! #[allow(clippy::unwrap_used)] // Missing reason parameter and missing preceding comment
//! fn read_config() { ... }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! // Invariant: The embedded config template is validated by build.rs and is guaranteed non-empty.
//! #[expect(clippy::unwrap_used, reason = "infallible static template lookup")]
//! fn read_config() { ... }
//! ```

use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule enforcing suppression hygiene: requiring `reason = "…"` and a preceding explanatory comment.
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

/// Visitor that inspects AST attributes to verify suppression hygiene.
struct SuppressVisitor<'a> {
    /// Context containing source text and preceding comment lookup helpers.
    ctx: &'a LintContext<'a>,
    /// Accumulated diagnostic findings.
    diagnostics: Vec<Diagnostic>,
}

impl<'ast> Visit<'ast> for SuppressVisitor<'_> {
    /// Inspects attributes for lint suppressions (`#[allow]` or `#[expect]`) and verifies hygiene.
    fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
        if let Some(diag) = check_suppression_attribute(self.ctx, attr) {
            self.diagnostics.push(diag);
        }

        visit::visit_attribute(self, attr);
    }
}

/// Checks whether an attribute is an unhygienic lint suppression, returning a diagnostic if non-compliant.
fn check_suppression_attribute(ctx: &LintContext<'_>, attr: &syn::Attribute) -> Option<Diagnostic> {
    if !is_lint_suppression_attr(attr) {
        return None;
    }

    let has_reason = has_reason_parameter(attr);
    let start_line = attr.span().start().line;
    let has_comment = ctx.has_preceding_comment(start_line);
    let span = ctx.to_span(attr.span());

    match (has_reason, has_comment) {
        (false, false) => Some(
            Diagnostic::new(
                "opinionated::clippy_suppression_hygiene",
                Severity::Warning,
                "Lint suppression attribute lacks both a 'reason = \"...\"' parameter and an explanatory code comment on the preceding line.",
            )
            .with_span(span)
            .with_suggested_fix("Add reason = \"...\" to the attribute and document the rationale in a comment directly above it."),
        ),
        (false, true) => Some(
            Diagnostic::new(
                "opinionated::clippy_suppression_hygiene",
                Severity::Warning,
                "Lint suppression attribute lacks a 'reason = \"...\"' parameter explaining why it is necessary.",
            )
            .with_span(span)
            .with_suggested_fix("Add reason = \"...\" parameter to the suppression attribute."),
        ),
        (true, false) => Some(
            Diagnostic::new(
                "opinionated::clippy_suppression_hygiene",
                Severity::Warning,
                "Lint suppression attribute lacks an accompanying code comment on the preceding line explaining why the lint cannot be resolved.",
            )
            .with_span(span)
            .with_suggested_fix("Add an explanatory '// ...' comment on the line immediately preceding the attribute."),
        ),
        (true, true) => None,
    }
}

/// Returns true if the attribute is `#[allow(...)]` or `#[expect(...)]`.
fn is_lint_suppression_attr(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("allow") || attr.path().is_ident("expect")
}

/// Checks whether the suppression attribute contains a `reason = "..."` parameter.
fn has_reason_parameter(attr: &syn::Attribute) -> bool {
    let mut has_reason = false;
    let _result = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("reason") {
            has_reason = true;
        }
        Ok(())
    });
    has_reason
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
