use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Rule banning `Box<dyn std::error::Error>` in production code return types.
pub struct NoBoxedDynErrorRule;

impl Rule for NoBoxedDynErrorRule {
    fn name(&self) -> &'static str {
        "opinionated::no_boxed_dyn_error"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut visitor = BoxedDynErrorVisitor {
            ctx,
            diagnostics: Vec::new(),
            in_cfg_test: ctx.is_test_file(),
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

struct BoxedDynErrorVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    in_cfg_test: bool,
}

impl<'ast> Visit<'ast> for BoxedDynErrorVisitor<'_> {
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

        let prev = self.in_cfg_test;
        if is_cfg_test {
            self.in_cfg_test = true;
        }

        visit::visit_item_mod(self, item_mod);
        self.in_cfg_test = prev;
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        self.check_fn_sig(&item_fn.sig, &item_fn.attrs);
        visit::visit_item_fn(self, item_fn);
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        self.check_fn_sig(&impl_fn.sig, &impl_fn.attrs);
        visit::visit_impl_item_fn(self, impl_fn);
    }
}

impl BoxedDynErrorVisitor<'_> {
    fn check_fn_sig(&mut self, sig: &syn::Signature, attrs: &[syn::Attribute]) {
        if self.in_cfg_test {
            return;
        }

        let fn_name = sig.ident.to_string();

        // Exempt main in binary entry points
        if fn_name == "main" {
            return;
        }

        // Exempt test functions
        let is_test = attrs.iter().any(|attr| {
            attr.path().is_ident("test")
                || attr
                    .path()
                    .segments
                    .last()
                    .map(|s| s.ident == "test")
                    .unwrap_or(false)
        });

        if is_test {
            return;
        }

        if let syn::ReturnType::Type(_, ty) = &sig.output
            && contains_boxed_dyn_error(ty)
        {
            let span = self.ctx.to_span(sig.output.span());
            self.diagnostics.push(
                Diagnostic::new(
                    "opinionated::no_boxed_dyn_error",
                    Severity::Error,
                    format!(
                        "Function '{fn_name}' returns 'Box<dyn Error>'. Return a concrete domain error enum deriving 'thiserror::Error' instead."
                    ),
                )
                .with_span(span)
                .with_suggested_fix("Return a concrete domain error enum deriving 'thiserror::Error' with appropriate '#[from]' conversions."),
            );
        }
    }
}

fn contains_boxed_dyn_error(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(type_path) => {
            for segment in &type_path.path.segments {
                if segment.ident == "Box"
                    && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
                {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(inner_ty) = arg
                            && is_dyn_error_trait(inner_ty)
                        {
                            return true;
                        }
                    }
                }

                // Recursively inspect any nested generic type arguments
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(inner_ty) = arg
                            && contains_boxed_dyn_error(inner_ty)
                        {
                            return true;
                        }
                    }
                }
            }
            false
        }
        syn::Type::Tuple(tup) => tup.elems.iter().any(contains_boxed_dyn_error),
        syn::Type::Reference(r) => contains_boxed_dyn_error(&r.elem),
        _ => false,
    }
}

fn is_dyn_error_trait(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::TraitObject(to) => to.bounds.iter().any(|bound| match bound {
            syn::TypeParamBound::Trait(trait_bound) => trait_bound
                .path
                .segments
                .last()
                .map(|s| s.ident == "Error")
                .unwrap_or(false),
            _ => false,
        }),
        syn::Type::Path(tp) => tp
            .path
            .segments
            .last()
            .map(|s| s.ident == "Error")
            .unwrap_or(false),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn boxed_dyn_error_in_fn_signature_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn run() -> Result<(), Box<dyn std::error::Error>> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/engine.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBoxedDynErrorRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::no_boxed_dyn_error"));
        assert_that!(
            &diag.message,
            contains_substring("Function 'run' returns 'Box<dyn Error>'")
        );
        Ok(())
    }

    #[googletest::test]
    fn boxed_dyn_error_with_send_sync_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn execute() -> Result<String, Box<dyn std::error::Error + Send + Sync>> { Ok(String::new()) }\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBoxedDynErrorRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(diag.severity, eq(Severity::Error));
        Ok(())
    }

    #[googletest::test]
    fn concrete_domain_error_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "pub fn run() -> Result<(), ConfigError> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/engine.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBoxedDynErrorRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn boxed_dyn_error_in_main_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = "fn main() -> Result<(), Box<dyn std::error::Error>> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/main.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBoxedDynErrorRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn boxed_dyn_error_in_test_fn_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source =
            "#[test]\nfn run_test() -> Result<(), Box<dyn std::error::Error>> { Ok(()) }\n";
        let ctx = LintContext::new(Path::new("src/lib.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = NoBoxedDynErrorRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
