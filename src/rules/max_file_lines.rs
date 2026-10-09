//! Rule: `purist::max_file_lines`
//!
//! # What This Rule Does
//! Enforces upper boundaries on the length of Rust source files.
//! It flags files that exceed recommended production or total line length limits,
//! encouraging developers to decompose sprawling files into smaller, cohesive submodules.
//!
//! # Why This Rule Exists
//! Monolithic source files (files spanning hundreds or thousands of lines) degrade codebase
//! maintainability, impede code reviews, complicate version control diffing, and lengthen compile
//! times. Large files often indicate missing architectural boundaries or mixed responsibilities.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! // In parser.rs (1200+ production lines):
//! // A single file implementing lexing, parsing, AST lowering, validation, and serialization.
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! // Decomposed into submodules:
//! // parser/mod.rs
//! // parser/lexer.rs
//! // parser/ast.rs
//! // parser/validation.rs
//! ```

use super::common::count_production_lines;
use crate::diagnostics::{Diagnostic, Severity, Span};
use crate::engine::{LintContext, Rule};

/// Default maximum allowed lines of production code in a single file (excluding `#[cfg(test)]`).
pub const DEFAULT_MAX_PRODUCTION_LINES: usize = 600;

/// Default maximum allowed total lines in a single file (including tests or pure test files).
pub const DEFAULT_MAX_TOTAL_LINES: usize = 1000;

/// Rule enforcing boundaries on source file line lengths.
pub struct MaxFileLinesRule;

impl Rule for MaxFileLinesRule {
    fn name(&self) -> &'static str {
        "purist::max_file_lines"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let total_lines = ctx.source().lines().count();
        let file_name = ctx
            .file_path()
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown");

        let max_production_lines = ctx
            .config()
            .and_then(|c| c.max_file_lines.max_production_lines)
            .unwrap_or(DEFAULT_MAX_PRODUCTION_LINES);

        let max_total_lines = ctx
            .config()
            .and_then(|c| c.max_file_lines.max_total_lines)
            .unwrap_or(DEFAULT_MAX_TOTAL_LINES);

        if ctx.is_test_file() {
            // Check test file against total lines limit
            if total_lines > max_total_lines {
                let first_line_span = Span::new(ctx.file_path(), 1, 1, 1, 1);
                diagnostics.push(
                    Diagnostic::new(
                        self.name(),
                        Severity::Warning,
                        format!(
                            "Test file '{file_name}' exceeds maximum length ({total_lines} lines, limit is {max_total_lines}). Split large test suites into focused sub-modules or separate integration test files.",
                        ),
                    )
                    .with_span(first_line_span)
                    .with_suggested_fix("Split tests across multiple integration test files or helper modules."),
                );
            }
            return diagnostics;
        }

        // For production files: check production lines count (excluding #[cfg(test)])
        let prod_lines = count_production_lines(file, ctx.source());
        if prod_lines > max_production_lines {
            let first_line_span = Span::new(ctx.file_path(), 1, 1, 1, 1);
            diagnostics.push(
                Diagnostic::new(
                    self.name(),
                    Severity::Warning,
                    format!(
                        "Source file '{file_name}' exceeds maximum production length ({prod_lines} production lines, limit is {max_production_lines}). Decompose into smaller, cohesive submodules.",
                    ),
                )
                .with_span(first_line_span)
                .with_suggested_fix("Extract distinct responsibilities into submodules declared with 'mod <name>;'."),
            );
        }

        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::{LintConfig, MaxFileLinesConfig};
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn small_file_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let code = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/math.rs"), code);
        let rule = MaxFileLinesRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn production_file_exceeding_production_limit_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let code = "// prod line\n".repeat(650);
        let file = syn::parse_file(&code)?;
        let ctx = LintContext::new(Path::new("src/large.rs"), &code);
        let rule = MaxFileLinesRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            diag.message,
            contains_substring("exceeds maximum production length")
        );
        Ok(())
    }

    #[googletest::test]
    fn large_cfg_test_module_with_thousands_of_lines_does_not_exceed_limit()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut code = String::from("pub fn logic() -> i32 {\n    42\n}\n\n");
        code.push_str("#[cfg(test)]\nmod tests {\n    use super::*;\n");
        for i in 0..1500 {
            code.push_str(&format!(
                "    #[test]\n    fn test_case_{i}() {{\n        assert_eq!({i}, {i});\n    }}\n"
            ));
        }
        code.push_str("}\n");

        let file = syn::parse_file(&code)?;
        let prod_lines = count_production_lines(&file, &code);
        // Only 4 lines of production code above the 6,000+ line test module
        expect_that!(prod_lines, eq(4));

        let ctx = LintContext::new(Path::new("src/engine.rs"), &code);
        let rule = MaxFileLinesRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn configured_limits_in_cargo_toml_override_defaults() -> Result<(), Box<dyn std::error::Error>>
    {
        // 650 production lines exceeds default limit of 600
        let code = "// prod line\n".repeat(650);
        let file = syn::parse_file(&code)?;

        // Default context without config: flags violation
        let default_ctx = LintContext::new(Path::new("src/large.rs"), &code);
        let rule = MaxFileLinesRule;
        let diags_default = rule.check_file(&default_ctx, &file);
        assert_that!(diags_default.len(), eq(1));

        // Configured context with max_production_lines = 700: passes cleanly
        let config = LintConfig {
            max_file_lines: MaxFileLinesConfig {
                max_production_lines: Some(700),
                max_total_lines: None,
            },
            ..Default::default()
        };
        let configured_ctx =
            LintContext::new(Path::new("src/large.rs"), &code).with_config(&config);
        let diags_configured = rule.check_file(&configured_ctx, &file);
        assert_that!(diags_configured, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn integration_test_file_within_limit_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let code = "// test line\n".repeat(500);
        let file = syn::parse_file(&code)?;
        let ctx = LintContext::new(Path::new("tests/integration_test.rs"), &code);
        let rule = MaxFileLinesRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn integration_test_file_exceeding_total_limit_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let code = "// test line\n".repeat(1050);
        let file = syn::parse_file(&code)?;
        let ctx = LintContext::new(Path::new("tests/huge_integration_test.rs"), &code);
        let rule = MaxFileLinesRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            diag.message,
            contains_substring("Test file 'huge_integration_test.rs' exceeds maximum length")
        );
        Ok(())
    }
}
