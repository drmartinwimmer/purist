//! Rule: `purist::free_functions`
//!
//! # What This Rule Does
//! Detects stateless dummy structs (unit structs or structs with zero fields) that are used solely
//! as namespaces for associated functions and implement no traits.
//!
//! # Why This Rule Exists
//! In Rust, modules are first-class language constructs designed for namespacing and privacy boundaries.
//! Creating empty structs with only associated functions (and no instance methods taking `&self` or `self`,
//! and no trait implementations) is an unidiomatic pattern carried over from languages like Java or C#
//! where all functions must belong to a class. In Rust, functions that do not operate on state should
//! be idiomatic module-level free functions (`pub fn parse(...)`).
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! pub struct StringParser; // Stateless dummy struct used as a namespace
//!
//! impl StringParser {
//!     pub fn parse_int(s: &str) -> Result<i32, ParseIntError> {
//!         s.parse()
//!     }
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! // In parser.rs module:
//! pub fn parse_int(s: &str) -> Result<i32, ParseIntError> {
//!     s.parse()
//! }
//! ```

use super::common::extract_type_ident;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use std::collections::{HashMap, HashSet};
use syn::Fields;

/// Rule detecting stateless dummy structs used solely as static namespaces.
pub struct FreeFunctionsRule;

impl Rule for FreeFunctionsRule {
    fn name(&self) -> &'static str {
        "purist::free_functions"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        // 1. Identify all unit or empty structs in the file
        let empty_structs = collect_empty_structs(file);
        if empty_structs.is_empty() {
            return Vec::new();
        }

        // 2. Analyze implementations: traits, instance methods, and static functions
        let impl_analysis = analyze_struct_implementations(file, &empty_structs);

        // 3. Generate findings for structs that have static functions but no instance methods or traits
        build_free_function_diagnostics(ctx, self.name(), empty_structs, impl_analysis)
    }
}

/// Stores implementation metrics for unit/empty structs.
struct StructImplAnalysis {
    implemented_traits: HashSet<String>,
    structs_with_instance_methods: HashSet<String>,
    structs_with_static_functions: HashMap<String, usize>,
}

/// Identifies all empty or unit structs defined in the file.
fn collect_empty_structs(file: &syn::File) -> HashMap<String, proc_macro2::Span> {
    let mut empty_structs = HashMap::new();
    for item in &file.items {
        if let syn::Item::Struct(item_struct) = item {
            let is_empty = match &item_struct.fields {
                Fields::Unit => true,
                Fields::Named(f) => f.named.is_empty(),
                Fields::Unnamed(f) => f.unnamed.is_empty(),
            };
            if is_empty {
                empty_structs.insert(item_struct.ident.to_string(), item_struct.ident.span());
            }
        }
    }
    empty_structs
}

/// Analyzes impl blocks for empty structs, recording instance methods, static methods, and trait implementations.
fn analyze_struct_implementations(
    file: &syn::File,
    empty_structs: &HashMap<String, proc_macro2::Span>,
) -> StructImplAnalysis {
    let mut implemented_traits = HashSet::new();
    let mut structs_with_instance_methods = HashSet::new();
    let mut structs_with_static_functions = HashMap::new();

    for item in &file.items {
        let syn::Item::Impl(item_impl) = item else {
            continue;
        };

        let Some(ident) = extract_type_ident(&item_impl.self_ty) else {
            continue;
        };
        let struct_name = ident.to_string();

        if !empty_structs.contains_key(&struct_name) {
            continue;
        }

        if item_impl.trait_.is_some() {
            implemented_traits.insert(struct_name);
            continue;
        }

        for impl_item in &item_impl.items {
            let syn::ImplItem::Fn(fn_item) = impl_item else {
                continue;
            };

            if fn_item.sig.receiver().is_some() {
                structs_with_instance_methods.insert(struct_name.clone());
            } else {
                *structs_with_static_functions
                    .entry(struct_name.clone())
                    .or_insert(0) += 1;
            }
        }
    }

    StructImplAnalysis {
        implemented_traits,
        structs_with_instance_methods,
        structs_with_static_functions,
    }
}

/// Produces diagnostics for structs that act strictly as function namespaces.
fn build_free_function_diagnostics(
    ctx: &LintContext<'_>,
    rule_name: &'static str,
    empty_structs: HashMap<String, proc_macro2::Span>,
    analysis: StructImplAnalysis,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (struct_name, span) in empty_structs {
        let static_fn_count = analysis
            .structs_with_static_functions
            .get(&struct_name)
            .copied()
            .unwrap_or(0);
        let has_instance_methods = analysis
            .structs_with_instance_methods
            .contains(&struct_name);
        let implements_traits = analysis.implemented_traits.contains(&struct_name);

        if static_fn_count > 0 && !has_instance_methods && !implements_traits {
            diagnostics.push(
                Diagnostic::new(
                    rule_name,
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

    diagnostics.sort_by_key(|d| d.span.as_ref().map(|s| s.start_line).unwrap_or(0));
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn dummy_namespace_struct_is_flagged() -> Result<(), Box<dyn std::error::Error>> {
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
        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::free_functions"));
        assert_that!(
            &diag.message,
            contains_substring("Struct 'Parser' is stateless")
        );
        Ok(())
    }

    #[googletest::test]
    fn struct_with_receiver_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
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
    fn struct_implementing_trait_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
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
    fn struct_with_fields_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
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
