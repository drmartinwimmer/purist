use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule flagging unstructured string error types (Result<T, String> or Result<T, &str>) in production functions.
pub struct ErrorTypesRule;

impl Rule for ErrorTypesRule {
    fn name(&self) -> &'static str {
        "opinionated::error_types"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let mut visitor = ErrorTypesVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_test_scope: false,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

struct ErrorTypesVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_test_scope: bool,
}

impl<'ast> Visit<'ast> for ErrorTypesVisitor<'_> {
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

        if !self.in_test_scope && !is_test {
            self.check_signature(&item_fn.sig.ident.to_string(), &item_fn.sig);
        }

        let previous_test_scope = self.in_test_scope;
        if is_test {
            self.in_test_scope = true;
        }

        visit::visit_item_fn(self, item_fn);
        self.in_test_scope = previous_test_scope;
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        if !self.in_test_scope {
            self.check_signature(&impl_fn.sig.ident.to_string(), &impl_fn.sig);
        }
        visit::visit_impl_item_fn(self, impl_fn);
    }

    fn visit_trait_item_fn(&mut self, trait_fn: &'ast syn::TraitItemFn) {
        if !self.in_test_scope {
            self.check_signature(&trait_fn.sig.ident.to_string(), &trait_fn.sig);
        }
        visit::visit_trait_item_fn(self, trait_fn);
    }
}

impl ErrorTypesVisitor<'_> {
    fn check_signature(&mut self, fn_name: &str, sig: &syn::Signature) {
        let return_type = match &sig.output {
            syn::ReturnType::Type(_, ty) => ty.as_ref(),
            syn::ReturnType::Default => return,
        };

        if let Some((error_ty, err_desc)) = detect_string_error_type(return_type) {
            let span = self.ctx.to_span(error_ty.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "opinionated::error_types",
                    Severity::Warning,
                    format!(
                        "Function '{fn_name}' returns unstructured error type '{err_desc}'. Use structured error enums via thiserror or anyhow::Result."
                    ),
                )
                .with_span(span)
                .with_suggested_fix("Define a dedicated error enum deriving 'thiserror::Error' or use 'anyhow::Result'.".to_string()),
            );
        }
    }
}

/// Detects if a return type is Result<T, String> or Result<T, &str>.
fn detect_string_error_type(ty: &syn::Type) -> Option<(&syn::Type, String)> {
    let type_path = match ty {
        syn::Type::Path(p) => p,
        _ => return None,
    };

    let last_segment = type_path.path.segments.last()?;
    if last_segment.ident != "Result" {
        return None;
    }

    let args = match &last_segment.arguments {
        syn::PathArguments::AngleBracketed(ab) => &ab.args,
        _ => return None,
    };

    if args.len() != 2 {
        return None;
    }

    let err_arg = match &args[1] {
        syn::GenericArgument::Type(t) => t,
        _ => return None,
    };

    if is_string_type(err_arg) {
        return Some((err_arg, "String".to_string()));
    }

    if is_str_ref_type(err_arg) {
        return Some((err_arg, "&str".to_string()));
    }

    None
}

fn is_string_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(p) = ty
        && let Some(seg) = p.path.segments.last()
    {
        return seg.ident == "String";
    }
    false
}

fn is_str_ref_type(ty: &syn::Type) -> bool {
    if let syn::Type::Reference(r) = ty
        && let syn::Type::Path(p) = &*r.elem
        && let Some(seg) = p.path.segments.last()
    {
        return seg.ident == "str";
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn result_string_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn compute() -> Result<i32, String> { Ok(42) }\n";
        let ctx = LintContext::new(Path::new("src/compute.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ErrorTypesRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::error_types"));
        assert_that!(
            &diag.message,
            contains_substring("returns unstructured error type 'String'")
        );
        Ok(())
    }

    #[googletest::test]
    fn result_str_ref_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn lookup() -> Result<usize, &'static str> { Ok(0) }\n";
        let ctx = LintContext::new(Path::new("src/lookup.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ErrorTypesRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::error_types"));
        assert_that!(
            &diag.message,
            contains_substring("returns unstructured error type '&str'")
        );
        Ok(())
    }

    #[googletest::test]
    fn result_custom_error_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn execute() -> Result<(), MyError> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/exec.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ErrorTypesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn result_string_in_test_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "#[test]\nfn parse_action() -> Result<(), String> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ErrorTypesRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
