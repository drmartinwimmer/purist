use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::visit::{self, Visit};

/// Rule detecting unanchored relative path operations in non-test code.
pub struct PathResolutionRule;

impl Rule for PathResolutionRule {
    fn name(&self) -> &'static str {
        "opinionated::path_resolution"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut visitor = PathVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_test_scope: false,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

struct PathVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_test_scope: bool,
}

impl<'ast> Visit<'ast> for PathVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let is_test = item_mod.attrs.iter().any(|attr| {
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

        let previous_test_scope = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }

        visit::visit_item_mod(self, item_mod);
        self.in_test_scope = previous_test_scope;
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

        let previous_test_scope = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }

        visit::visit_item_fn(self, item_fn);
        self.in_test_scope = previous_test_scope;
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if !self.in_test_scope {
            self.check_call(&call.func, &call.args);
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if !self.in_test_scope {
            self.check_method_call(call);
        }
        visit::visit_expr_method_call(self, call);
    }
}

impl PathVisitor<'_> {
    fn check_call(&mut self, func: &syn::Expr, args: &Punctuated<syn::Expr, Comma>) {
        let path = match func {
            syn::Expr::Path(expr_path) => &expr_path.path,
            _ => return,
        };

        let path_str = path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");

        let is_target_call = path_str == "Path::new"
            || path_str == "std::path::Path::new"
            || path_str == "PathBuf::from"
            || path_str == "std::path::PathBuf::from"
            || path_str == "fs::read"
            || path_str == "std::fs::read"
            || path_str == "fs::read_to_string"
            || path_str == "std::fs::read_to_string"
            || path_str == "fs::write"
            || path_str == "std::fs::write"
            || path_str == "File::open"
            || path_str == "std::fs::File::open"
            || path_str == "File::create"
            || path_str == "std::fs::File::create";

        if !is_target_call {
            return;
        }

        if let Some(first_arg) = args.first() {
            self.inspect_path_arg(first_arg);
        }
    }

    fn check_method_call(&mut self, call: &syn::ExprMethodCall) {
        let method_name = call.method.to_string();
        if method_name == "join" {
            // If joining on a bare relative path literal directly (e.g. "relative/dir".join(...))
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit_str),
                ..
            }) = &*call.receiver
            {
                self.inspect_lit_str(lit_str);
            }
        }
    }

    fn inspect_path_arg(&mut self, arg: &syn::Expr) {
        if let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit_str),
            ..
        }) = arg
        {
            self.inspect_lit_str(lit_str);
        }
    }

    fn inspect_lit_str(&mut self, lit_str: &syn::LitStr) {
        let val = lit_str.value();
        if is_unanchored_relative_path(&val) {
            let span = self.ctx.to_span(lit_str.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "opinionated::path_resolution",
                    Severity::Warning,
                    format!(
                        "Unanchored relative path '{val}' in '{}'. Relative paths break when executed outside the crate root.",
                        self.ctx.file_path().display()
                    ),
                )
                .with_span(span)
                .with_suggested_fix(format!(
                    "Anchor path using 'Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"{val}\")' or workspace root."
                )),
            );
        }
    }
}

/// Checks whether a string literal is an unanchored relative path.
fn is_unanchored_relative_path(path_str: &str) -> bool {
    let trimmed = path_str.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Absolute Unix / Windows paths
    if trimmed.starts_with('/') || trimmed.starts_with('\\') {
        return false;
    }
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2
        && bytes.first().is_some_and(|b| b.is_ascii_alphabetic())
        && bytes.get(1) == Some(&b':')
    {
        return false;
    }

    // Protocol URLs
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("file://")
    {
        return false;
    }

    // Command-line flags
    if trimmed.starts_with('-') {
        return false;
    }

    // Must look like a path or file
    trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.ends_with(".json")
        || trimmed.ends_with(".toml")
        || trimmed.ends_with(".yaml")
        || trimmed.ends_with(".txt")
        || trimmed.ends_with(".rs")
        || trimmed.ends_with(".md")
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn relative_path_new_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use std::path::Path;

pub fn load() {
    let _ = Path::new("config/settings.json");
}
"#;
        let ctx = LintContext::new(Path::new("src/config.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PathResolutionRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::path_resolution"));
        assert_that!(
            &diag.message,
            contains_substring("Unanchored relative path 'config/settings.json'")
        );
        Ok(())
    }

    #[googletest::test]
    fn relative_fs_read_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn read_data() {
    let _ = std::fs::read_to_string("data.toml");
}
"#;
        let ctx = LintContext::new(Path::new("src/loader.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PathResolutionRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::path_resolution"));
        Ok(())
    }

    #[googletest::test]
    fn absolute_path_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn load() {
    let _ = std::path::Path::new("/etc/config.json");
}
"#;
        let ctx = LintContext::new(Path::new("src/config.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PathResolutionRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn path_in_test_fn_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn sample_action() {
    let _ = std::path::Path::new("test_data.json");
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PathResolutionRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn path_in_cfg_test_mod_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[cfg(test)]
mod tests {
    use std::path::Path;
    fn helper() {
        let _ = Path::new("dummy.txt");
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PathResolutionRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn path_join_with_relative_component_on_variable_is_permitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
use std::path::{Path, PathBuf};

pub fn find_manifest(base: &Path) -> PathBuf {
    base.join("Cargo.toml")
}
"#;
        let ctx = LintContext::new(Path::new("src/config.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = PathResolutionRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
