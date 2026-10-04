use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging raw `Command::new` invocations outside dedicated command/tool modules.
pub struct CentralizedCommandsRule;

impl Rule for CentralizedCommandsRule {
    fn name(&self) -> &'static str {
        "opinionated::centralized_command_execution"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if is_exempt_path(ctx) {
            return Vec::new();
        }

        let mut visitor = CommandVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_test_scope: false,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

fn is_exempt_path(ctx: &LintContext<'_>) -> bool {
    if ctx.is_test_file() {
        return true;
    }

    let path_str = ctx.file_path().to_string_lossy();
    path_str.contains("/tools/")
        || path_str.contains("/commands/")
        || path_str.contains("/cmd/")
        || path_str.ends_with("_tool.rs")
        || path_str.ends_with("_command.rs")
        || path_str.ends_with("/tool.rs")
        || path_str.ends_with("/command.rs")
}

struct CommandVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_test_scope: bool,
}

impl<'ast> Visit<'ast> for CommandVisitor<'_> {
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

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if !self.in_test_scope
            && let syn::Expr::Path(expr_path) = &*call.func
            && is_command_new_path(&expr_path.path)
        {
            let span = self.ctx.to_span(call.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "opinionated::centralized_command_execution",
                    Severity::Warning,
                    "Direct invocation of 'Command::new' outside dedicated command/tool module. Encapsulate external process execution in a dedicated tool struct.",
                )
                .with_span(span)
                .with_suggested_fix("Encapsulate command execution and stdout parsing in a dedicated tool builder struct in a 'tools' or 'commands' module."),
            );
        }

        visit::visit_expr_call(self, call);
    }
}

fn is_command_new_path(path: &syn::Path) -> bool {
    let segments: Vec<&syn::PathSegment> = path.segments.iter().collect();
    if segments.len() < 2 {
        return false;
    }

    let last = segments.last();
    let second_to_last = segments.get(segments.len().saturating_sub(2));

    if let (Some(l), Some(s)) = (last, second_to_last) {
        l.ident == "new" && s.ident == "Command"
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn command_new_in_service_module_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn check_git() {
    let _ = std::process::Command::new("git").status();
}
"#;
        let ctx = LintContext::new(Path::new("src/services/git.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CentralizedCommandsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::centralized_command_execution"));
        Ok(())
    }

    #[googletest::test]
    fn command_new_in_tools_module_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub fn run_cargo() {
    let _ = std::process::Command::new("cargo").status();
}
"#;
        let ctx = LintContext::new(Path::new("src/tools/cargo.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CentralizedCommandsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn command_new_in_test_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[test]
fn execute_git_works() {
    let _ = Command::new("git").status();
}
"#;
        let ctx = LintContext::new(Path::new("src/services/git.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = CentralizedCommandsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
