use crate::engine::{LintContext, Rule};
use code_review_diagnostics::{Diagnostic, Severity};
use std::collections::{HashMap, HashSet};

/// Rule detecting stateless dummy structs used solely as static namespaces.
pub struct FreeFunctionsRule;

impl Rule for FreeFunctionsRule {
    fn name(&self) -> &'static str {
        "opinionated::free_functions"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // 1. Identify unit / empty structs: struct_name -> ident span
        let mut empty_structs: HashMap<String, proc_macro2::Span> = HashMap::new();
        for item in &file.items {
            if let syn::Item::Struct(item_struct) = item {
                let is_empty = match &item_struct.fields {
                    syn::Fields::Unit => true,
                    syn::Fields::Named(f) => f.named.is_empty(),
                    syn::Fields::Unnamed(f) => f.unnamed.is_empty(),
                };
                if is_empty {
                    empty_structs.insert(item_struct.ident.to_string(), item_struct.ident.span());
                }
            }
        }

        if empty_structs.is_empty() {
            return diagnostics;
        }

        // 2. Track trait implementations and method presence per struct
        let mut implemented_traits: HashSet<String> = HashSet::new();
        let mut structs_with_instance_methods: HashSet<String> = HashSet::new();
        let mut structs_with_static_functions: HashMap<String, usize> = HashMap::new();

        for item in &file.items {
            if let syn::Item::Impl(item_impl) = item {
                let struct_name = match extract_type_ident(&item_impl.self_ty) {
                    Some(name) => name,
                    None => continue,
                };

                if !empty_structs.contains_key(&struct_name) {
                    continue;
                }

                if item_impl.trait_.is_some() {
                    implemented_traits.insert(struct_name);
                    continue;
                }

                for impl_item in &item_impl.items {
                    if let syn::ImplItem::Fn(fn_item) = impl_item {
                        if fn_item.sig.receiver().is_some() {
                            structs_with_instance_methods.insert(struct_name.clone());
                        } else {
                            *structs_with_static_functions
                                .entry(struct_name.clone())
                                .or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        // 3. Flag structs that only have static functions, no instance methods, and no trait implementations
        for (struct_name, span) in empty_structs {
            let static_fn_count = structs_with_static_functions
                .get(&struct_name)
                .copied()
                .unwrap_or(0);
            let has_instance_methods = structs_with_instance_methods.contains(&struct_name);
            let implements_traits = implemented_traits.contains(&struct_name);

            if static_fn_count > 0 && !has_instance_methods && !implements_traits {
                diagnostics.push(
                    Diagnostic::new(
                        self.name(),
                        Severity::Warning,
                        format!(
                            "Struct '{struct_name}' is stateless and used solely as a namespace for functions. Replace with idiomatic free functions in the module."
                        ),
                    )
                    .with_span(ctx.to_span(span))
                    .with_suggested_fix(format!(
                        "Remove struct '{struct_name}' and export its functions as top-level free functions."
                    )),
                );
            }
        }

        // Sort diagnostics by start line for deterministic output
        diagnostics.sort_by_key(|d| d.span.as_ref().map(|s| s.start_line).unwrap_or(0));
        diagnostics
    }
}

/// Extracts the identifier of a type if it is a simple path.
fn extract_type_ident(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(type_path) = ty
        && type_path.qself.is_none()
    {
        return type_path.path.segments.last().map(|s| s.ident.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn test_dummy_namespace_struct_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Parser;

impl Parser {
    pub fn parse(input: &str) -> usize {
        input.len()
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/parser.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = FreeFunctionsRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(1));
        assert_that!(diags[0].rule.as_str(), eq("opinionated::free_functions"));
        assert_that!(
            diags[0].message,
            contains_substring("Struct 'Parser' is stateless")
        );
        Ok(())
    }

    #[googletest::test]
    fn test_struct_with_receiver_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Service;

impl Service {
    pub fn start(&self) {}
}
"#;
        let ctx = LintContext::new(Path::new("src/service.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = FreeFunctionsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn test_struct_implementing_trait_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Visitor;

pub trait Visit {
    fn visit(&self);
}

impl Visit for Visitor {
    fn visit(&self) {}
}

impl Visitor {
    pub fn create() -> Self { Visitor }
}
"#;
        let ctx = LintContext::new(Path::new("src/visitor.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = FreeFunctionsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn test_struct_with_fields_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Config {
    pub timeout: u64,
}

impl Config {
    pub fn default_config() -> Self {
        Self { timeout: 10 }
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/config.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = FreeFunctionsRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
