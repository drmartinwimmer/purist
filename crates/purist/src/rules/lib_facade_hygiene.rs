//! Rule: `purist::lib_facade_hygiene`
//!
//! # What This Rule Does
//! Enforces that library root files (`src/lib.rs`) remain clean, concise architectural facades.
//! It flags `lib.rs` files that exceed line length limits or define substantial function logic
//! directly in the root file rather than delegating to dedicated submodules.
//!
//! # Why This Rule Exists
//! In idiomatic Rust library design, `lib.rs` acts as the public facade, module index, and
//! re-export boundary of the crate. When `lib.rs` accumulates heavy domain logic, command
//! implementations, or monolithic code blocks, it degrades architectural clarity, makes the
//! public API harder to inspect, and leads to large, difficult-to-review diffs.
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! // In src/lib.rs (250+ lines with inline business logic):
//! pub fn process_data(input: &str) -> Result<Output, Error> {
//!     // 30 lines of complex processing directly in lib.rs
//!     ...
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! // In src/lib.rs:
//! mod processing;
//! pub use processing::{process_data, Output};
//! ```

use crate::diagnostics::{Diagnostic, Severity, Span};
use crate::engine::{LintContext, Rule};
use syn::spanned::Spanned;
use syn::{Item, ItemFn};

/// Default maximum allowed line count for production code in `lib.rs`.
pub const DEFAULT_MAX_LIB_PRODUCTION_LINES: usize = 200;

/// Default maximum lines for a free function body defined directly in `lib.rs`.
pub const DEFAULT_MAX_LIB_FN_LINES: usize = 20;

/// Rule enforcing concise, modular library root files (`lib.rs`).
pub struct LibFacadeHygieneRule;

impl Rule for LibFacadeHygieneRule {
    fn name(&self) -> &'static str {
        "purist::lib_facade_hygiene"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let is_lib_file = ctx
            .file_path()
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name == "lib.rs")
            .unwrap_or(false);

        if !is_lib_file || ctx.is_test_file() {
            return Vec::new();
        }

        let max_prod_lines = ctx
            .config()
            .and_then(|c| c.lib_facade_hygiene.max_production_lines)
            .unwrap_or(DEFAULT_MAX_LIB_PRODUCTION_LINES);

        let max_fn_lines = ctx
            .config()
            .and_then(|c| c.lib_facade_hygiene.max_fn_lines)
            .unwrap_or(DEFAULT_MAX_LIB_FN_LINES);

        let mut diagnostics = Vec::new();

        // 1. Check production line count (excluding #[cfg(test)] test modules via AST spans)
        let prod_lines = count_production_lines(file, ctx.source());
        if prod_lines > max_prod_lines {
            let first_line_span = Span::new(ctx.file_path(), 1, 1, 1, 1);
            diagnostics.push(
                Diagnostic::new(
                    self.name(),
                    Severity::Warning,
                    format!(
                        "Library root 'lib.rs' is too large ({prod_lines} production lines, limit is {max_prod_lines}). Keep 'lib.rs' as a concise facade index.",
                    ),
                )
                .with_span(first_line_span)
                .with_suggested_fix("Move domain logic, command definitions, or helpers into dedicated submodules and re-export them from lib.rs."),
            );
        }

        // 2. Check for large free functions directly in lib.rs outside cfg(test)
        for item in &file.items {
            if let Item::Fn(item_fn) = item {
                if is_test_item(item_fn) {
                    continue;
                }

                let span = item_fn.span();
                let start_line = span.start().line;
                let end_line = span.end().line;
                let fn_lines = end_line.saturating_sub(start_line) + 1;

                if fn_lines > max_fn_lines {
                    let fn_name = &item_fn.sig.ident;
                    diagnostics.push(
                        Diagnostic::new(
                            self.name(),
                            Severity::Warning,
                            format!(
                                "Function '{fn_name}' in 'lib.rs' is too long ({fn_lines} lines, limit is {max_fn_lines}). Functions in 'lib.rs' should be brief facade delegates.",
                            ),
                        )
                        .with_span(ctx.to_span(span))
                        .with_suggested_fix(format!("Move the implementation of '{fn_name}' into a dedicated submodule and re-export it.")),
                    );
                }
            }
        }

        diagnostics
    }
}

/// Counts lines of production code in source text, excluding any lines in `#[cfg(test)]` items.
pub fn count_production_lines(file: &syn::File, source: &str) -> usize {
    let test_spans = find_test_line_spans(file);
    if test_spans.is_empty() {
        return source.lines().count();
    }

    let mut prod_lines = 0;
    for (line_idx, _) in source.lines().enumerate() {
        let line_num = line_idx + 1;
        let in_test = test_spans
            .iter()
            .any(|&(start, end)| line_num >= start && line_num <= end);
        if !in_test {
            prod_lines += 1;
        }
    }
    prod_lines
}

fn find_test_line_spans(file: &syn::File) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    for item in &file.items {
        if is_test_ast_item(item) {
            let span = item.span();
            spans.push((span.start().line, span.end().line));
        }
    }
    spans
}

fn is_test_ast_item(item: &Item) -> bool {
    let attrs = match item {
        Item::Mod(m) => &m.attrs,
        Item::Fn(f) => &f.attrs,
        Item::Struct(s) => &s.attrs,
        Item::Enum(e) => &e.attrs,
        Item::Const(c) => &c.attrs,
        Item::Static(s) => &s.attrs,
        Item::Impl(i) => &i.attrs,
        Item::Trait(t) => &t.attrs,
        Item::Use(u) => &u.attrs,
        _ => return false,
    };
    attrs.iter().any(is_cfg_test_attr)
}

fn is_cfg_test_attr(attr: &syn::Attribute) -> bool {
    if attr.path().is_ident("test") {
        return true;
    }
    if attr.path().is_ident("cfg")
        && let syn::Meta::List(ref list) = attr.meta
    {
        return list.tokens.to_string().contains("test");
    }
    false
}

fn is_test_item(item_fn: &ItemFn) -> bool {
    item_fn.attrs.iter().any(is_cfg_test_attr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cargo::{LibFacadeHygieneConfig, LintConfig};
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn small_concise_lib_rs_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let code = r#"
            pub mod foo;
            pub mod bar;
            pub use foo::Foo;
            pub use bar::Bar;
        "#;
        let file = syn::parse_file(code)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), code);
        let rule = LibFacadeHygieneRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn non_lib_file_is_exempt() -> Result<(), Box<dyn std::error::Error>> {
        let code = "pub fn large() {\n".to_string() + &"    let x = 1;\n".repeat(50) + "}\n";
        let file = syn::parse_file(&code)?;
        let ctx = LintContext::new(Path::new("src/helpers.rs"), &code);
        let rule = LibFacadeHygieneRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn large_production_lib_rs_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let code = "pub mod sub;\n".to_string() + &"// line\n".repeat(250);
        let file = syn::parse_file(&code)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), &code);
        let rule = LibFacadeHygieneRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(diag.message, contains_substring("is too large"));
        Ok(())
    }

    #[googletest::test]
    fn large_function_in_lib_rs_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let fn_body = "    let a = 1;\n".repeat(25);
        let code = format!("pub fn heavy_worker() {{\n{fn_body}}}\n");
        let file = syn::parse_file(&code)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), &code);
        let rule = LibFacadeHygieneRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(
            diag.message,
            contains_substring("Function 'heavy_worker' in 'lib.rs' is too long")
        );
        Ok(())
    }

    #[googletest::test]
    fn tests_module_does_not_count_towards_production_line_limit()
    -> Result<(), Box<dyn std::error::Error>> {
        let prod_part = "pub mod sub;\n".to_string() + &"// prod line\n".repeat(50);
        let test_part =
            "#[cfg(test)]\nmod tests {\n".to_string() + &"    // test line\n".repeat(500) + "}\n";
        let code = prod_part + &test_part;
        let file = syn::parse_file(&code)?;
        let ctx = LintContext::new(Path::new("src/lib.rs"), &code);
        let rule = LibFacadeHygieneRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn large_cfg_test_module_with_thousands_of_lines_does_not_exceed_limit()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut code = String::from("pub mod facade;\npub use facade::Entrypoint;\n\n");
        code.push_str("#[cfg(test)]\nmod tests {\n    use super::*;\n");
        for i in 0..1500 {
            code.push_str(&format!(
                "    #[test]\n    fn test_case_{i}() {{\n        assert_eq!({i}, {i});\n    }}\n"
            ));
        }
        code.push_str("}\n");

        let file = syn::parse_file(&code)?;
        let prod_lines = count_production_lines(&file, &code);
        // Only 3 lines of production code above the 6,000+ line test module
        expect_that!(prod_lines, eq(3));

        let ctx = LintContext::new(Path::new("src/lib.rs"), &code);
        let rule = LibFacadeHygieneRule;
        let diags = rule.check_file(&ctx, &file);
        assert_that!(diags, is_empty());
        Ok(())
    }

    #[googletest::test]
    fn configured_limits_in_cargo_toml_override_defaults() -> Result<(), Box<dyn std::error::Error>>
    {
        // 250 lines exceeds the default limit of 200
        let code = "pub mod sub;\n".to_string() + &"// line\n".repeat(240);
        let file = syn::parse_file(&code)?;

        // Default context without config: fails
        let default_ctx = LintContext::new(Path::new("src/lib.rs"), &code);
        let rule = LibFacadeHygieneRule;
        let diags_default = rule.check_file(&default_ctx, &file);
        assert_that!(diags_default.len(), eq(1));

        // Context with configured max_production_lines = 300: passes cleanly
        let config = LintConfig {
            lib_facade_hygiene: LibFacadeHygieneConfig {
                max_production_lines: Some(300),
                max_fn_lines: None,
            },
            ..Default::default()
        };
        let configured_ctx = LintContext::new(Path::new("src/lib.rs"), &code).with_config(&config);
        let diags_configured = rule.check_file(&configured_ctx, &file);
        assert_that!(diags_configured, is_empty());
        Ok(())
    }
}
