//! Procedural macros enforcing consistency across rule files, `RuleId`, `RuleDiagnostic`,
//! per-rule `<Rule>Diagnostic` types, `<Rule>Rule` structs, and `PuristLintsConfig`.

use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{Attribute, Data, DeriveInput, Error, Field, Fields, Ident, LitStr, Meta, Type, Variant};

/// Injects `code(purist::<snake_name>)` (and default `severity(Warning)` if not explicitly specified)
/// into `#[diagnostic(...)]` on the given attribute list.
fn inject_diagnostic_code(
    attrs: &mut Vec<Attribute>,
    target_span: Span,
    snake_ident: &Ident,
) -> syn::Result<()> {
    for attr in attrs.iter_mut() {
        if !attr.path().is_ident("diagnostic") {
            continue;
        }
        match &attr.meta {
            Meta::List(list) => {
                let tokens_str = list.tokens.to_string();
                if tokens_str.contains("code(") {
                    return Err(Error::new_spanned(
                        attr,
                        "`#[rule_diagnostic]` automatically sets `code(purist::<rule_name>)` from the type name; omit manual `code(...)`",
                    ));
                }
                let existing = &list.tokens;
                if tokens_str.contains("severity(") {
                    *attr = syn::parse_quote!(#[diagnostic(code(purist::#snake_ident), #existing)]);
                } else {
                    *attr = syn::parse_quote!(
                        #[diagnostic(code(purist::#snake_ident), severity(Warning), #existing)]
                    );
                }
                return Ok(());
            }
            Meta::Path(_) => {
                *attr = syn::parse_quote!(
                    #[diagnostic(code(purist::#snake_ident), severity(Warning))]
                );
                return Ok(());
            }
            Meta::NameValue(_) => {
                return Err(Error::new_spanned(
                    attr,
                    "Expected `#[diagnostic(...)]` list attribute",
                ));
            }
        }
    }

    let new_attr: Attribute = syn::parse_quote_spanned! { target_span =>
        #[diagnostic(code(purist::#snake_ident), severity(Warning))]
    };
    attrs.push(new_attr);
    Ok(())
}

/// Ensures the given named fields contain a `span: Span` field and injects `#[source_code]` and `#[label]` on it.
fn inject_span_field_attrs(
    owner_ident: &Ident,
    fields: &mut Punctuated<Field, Comma>,
) -> syn::Result<()> {
    for field in fields.iter_mut() {
        if field.ident.as_ref().is_some_and(|id| id == "span") {
            if !field.attrs.iter().any(|a| a.path().is_ident("source_code")) {
                field.attrs.push(syn::parse_quote!(#[source_code]));
            }
            if !field.attrs.iter().any(|a| a.path().is_ident("label")) {
                field.attrs.push(syn::parse_quote!(#[label]));
            }
            return Ok(());
        }
    }
    Err(Error::new_spanned(
        owner_ident,
        format!("`{owner_ident}` must include a `span: Span` field for source location reporting"),
    ))
}

/// Injects `code(purist::<snake_name>)` and `#[source_code] #[label]` onto every variant of an enum.
fn inject_enum_variants_code_and_span(
    variants: &mut Punctuated<Variant, Comma>,
    snake_ident: &Ident,
) -> syn::Result<Vec<Ident>> {
    let mut variant_idents = Vec::with_capacity(variants.len());
    for variant in variants {
        inject_diagnostic_code(&mut variant.attrs, variant.ident.span(), snake_ident)?;
        let Fields::Named(fields_named) = &mut variant.fields else {
            return Err(Error::new_spanned(
                &variant.ident,
                format!(
                    "Variant `{}` must have named fields including `span: Span`",
                    variant.ident
                ),
            ));
        };
        inject_span_field_attrs(&variant.ident, &mut fields_named.named)?;
        variant_idents.push(variant.ident.clone());
    }
    Ok(variant_idents)
}

/// Attribute macro `#[rule_diagnostic]` for per-rule diagnostic structs and enums.
///
/// Enforces that:
/// 1. The type name is `<PascalRuleName>Diagnostic`.
/// 2. The type is defined in module `src/rules/<snake_rule_name>.rs`.
/// 3. `RuleId::<PascalRuleName>` exists in the rule registry.
/// 4. Every variant (or struct) has `#[diagnostic(code(purist::<snake_rule_name>), ...)]` and `span: Span`.
/// 5. Generates `impl crate::engine::Rule for <PascalRuleName>Rule` delegating to `<PascalRuleName>Rule::check_file`.
pub fn expand_rule_diagnostic(attr: TokenStream, mut input: DeriveInput) -> TokenStream {
    if !attr.is_empty() {
        return Error::new(
            Span::call_site(),
            "`#[rule_diagnostic]` does not take arguments; the rule name is derived from `<RuleName>Diagnostic`",
        )
        .to_compile_error()
        .into();
    }

    let diag_ident = input.ident.clone();
    let diag_name = diag_ident.to_string();
    let Some(pascal_str) = diag_name.strip_suffix("Diagnostic") else {
        return Error::new_spanned(
            &diag_ident,
            "Per-rule diagnostic type must be named `<RuleName>Diagnostic`",
        )
        .to_compile_error()
        .into();
    };

    if pascal_str.is_empty() {
        return Error::new_spanned(
            &diag_ident,
            "Per-rule diagnostic type must have a non-empty `<RuleName>` prefix before `Diagnostic`",
        )
        .to_compile_error()
        .into();
    }

    let snake_str = pascal_str.to_snake_case();
    if snake_str.to_upper_camel_case() != pascal_str {
        return Error::new_spanned(
            &diag_ident,
            format!(
                "Diagnostic type `{diag_name}` does not round-trip cleanly with snake_case `{snake_str}`"
            ),
        )
        .to_compile_error()
        .into();
    }

    let pascal_ident = Ident::new(pascal_str, diag_ident.span());
    let snake_ident = Ident::new(&snake_str, diag_ident.span());
    let rule_struct_ident = Ident::new(&format!("{pascal_str}Rule"), diag_ident.span());

    let span_getter_body: syn::Result<TokenStream2> = match &mut input.data {
        Data::Struct(data_struct) => {
            if let Err(err) =
                inject_diagnostic_code(&mut input.attrs, diag_ident.span(), &snake_ident)
            {
                return err.to_compile_error().into();
            }
            let Fields::Named(fields_named) = &mut data_struct.fields else {
                return Error::new_spanned(
                    &diag_ident,
                    format!(
                        "`{diag_ident}` must be a struct with named fields including `span: Span`"
                    ),
                )
                .to_compile_error()
                .into();
            };
            if let Err(err) = inject_span_field_attrs(&diag_ident, &mut fields_named.named) {
                return err.to_compile_error().into();
            }
            Ok(quote! { &self.span })
        }
        Data::Enum(data_enum) => {
            match inject_enum_variants_code_and_span(&mut data_enum.variants, &snake_ident) {
                Ok(v_idents) => Ok(quote! {
                    match self {
                        #( Self::#v_idents { span, .. } => span, )*
                    }
                }),
                Err(err) => Err(err),
            }
        }
        Data::Union(_) => Err(Error::new_spanned(
            &diag_ident,
            "`#[rule_diagnostic]` cannot be applied to unions",
        )),
    };

    let span_body = match span_getter_body {
        Ok(tokens) => tokens,
        Err(err) => return err.to_compile_error().into(),
    };

    let expected_mod_suffix = format!("::{snake_str}");

    let expanded = quote! {
        #input

        impl #diag_ident {
            /// Returns the canonical [`crate::rules::RuleId`] associated with this diagnostic.
            pub(crate) const fn rule_id(&self) -> crate::rules::RuleId {
                crate::rules::RuleId::#pascal_ident
            }

            /// Returns the source [`crate::diagnostics::Span`] where this diagnostic was triggered.
            pub const fn span(&self) -> &crate::diagnostics::Span {
                #span_body
            }
        }

        impl crate::engine::Rule for #rule_struct_ident {
            fn name(&self) -> &'static str {
                crate::rules::RuleId::#pascal_ident.qualified_name()
            }

            fn check_file(
                &self,
                ctx: &crate::engine::LintContext<'_>,
                file: &syn::File,
            ) -> Vec<crate::rules::RuleDiagnostic> {
                Self::check_file(self, ctx, file)
                    .into_iter()
                    .map(crate::rules::RuleDiagnostic::from)
                    .collect()
            }
        }

        const _: () = {
            let mod_bytes = module_path!().as_bytes();
            let suffix = #expected_mod_suffix.as_bytes();
            assert!(
                mod_bytes.len() >= suffix.len(),
                "Rule diagnostic module path is shorter than expected rule module name"
            );
            let (_, mut tail) = mod_bytes.split_at(mod_bytes.len() - suffix.len());
            let mut expected = suffix;
            while let ([a, rest_a @ ..], [b, rest_b @ ..]) = (tail, expected) {
                assert!(
                    *a == *b,
                    "Rule diagnostic type name `<RuleName>Diagnostic` must match its file/module name `src/rules/<rule_name>.rs`"
                );
                tail = rest_a;
                expected = rest_b;
            }
        };
    };

    expanded.into()
}

/// Checks whether a variant has `#[attr_name(transparent)]`.
fn has_transparent_attr(attrs: &[Attribute], attr_name: &str) -> bool {
    for attr in attrs {
        if !attr.path().is_ident(attr_name) {
            continue;
        }
        if let Meta::List(list) = &attr.meta
            && list.tokens.to_string().trim() == "transparent"
        {
            return true;
        }
    }
    false
}

/// Extracts all string literals from `#[rule_alias("...")]` attributes on a variant.
fn extract_rule_aliases(attrs: &[Attribute]) -> syn::Result<Vec<LitStr>> {
    let mut aliases = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("rule_alias") {
            continue;
        }
        let lit: LitStr = attr.parse_args()?;
        aliases.push(lit);
    }
    Ok(aliases)
}

struct RegistryEntry {
    pascal_ident: Ident,
    snake_ident: Ident,
    snake_lit: LitStr,
    rule_struct_ident: Ident,
    aliases: Vec<LitStr>,
}

fn parse_registry_variants(
    enum_ident: &Ident,
    variants: &Punctuated<Variant, Comma>,
) -> syn::Result<Vec<RegistryEntry>> {
    let mut entries = Vec::with_capacity(variants.len());
    for variant in variants {
        let v_ident = &variant.ident;
        let pascal_str = v_ident.to_string();
        let snake_str = pascal_str.to_snake_case();

        if !has_transparent_attr(&variant.attrs, "error")
            || !has_transparent_attr(&variant.attrs, "diagnostic")
        {
            return Err(Error::new_spanned(
                v_ident,
                format!(
                    "Variant `{pascal_str}` in `{enum_ident}` must be annotated with `#[error(transparent)]` and `#[diagnostic(transparent)]`"
                ),
            ));
        }

        let Fields::Unnamed(fields_unnamed) = &variant.fields else {
            return Err(Error::new_spanned(
                v_ident,
                format!(
                    "Variant `{pascal_str}` in `{enum_ident}` must be a single-element tuple variant `{pascal_str}(#[from] {pascal_str}Diagnostic)`"
                ),
            ));
        };

        if fields_unnamed.unnamed.len() != 1 {
            return Err(Error::new_spanned(
                v_ident,
                format!("Variant `{pascal_str}` must contain exactly 1 inner diagnostic type"),
            ));
        }

        let field = fields_unnamed.unnamed.first().unwrap();
        let has_from = field.attrs.iter().any(|a| a.path().is_ident("from"));
        if !has_from {
            return Err(Error::new_spanned(
                field,
                format!(
                    "Variant `{pascal_str}` inner type must have `#[from]` for automatic conversion into `{enum_ident}`"
                ),
            ));
        }

        let expected_diag_name = format!("{pascal_str}Diagnostic");
        let type_matches = match &field.ty {
            Type::Path(tp) => tp
                .path
                .segments
                .last()
                .is_some_and(|seg| seg.ident == expected_diag_name.as_str()),
            _ => false,
        };
        if !type_matches {
            return Err(Error::new_spanned(
                &field.ty,
                format!(
                    "Variant `{pascal_str}` must wrap `{expected_diag_name}` to maintain 1-to-1 naming consistency"
                ),
            ));
        }

        let aliases = extract_rule_aliases(&variant.attrs)?;
        let snake_ident = Ident::new(&snake_str, v_ident.span());
        let rule_struct_ident = Ident::new(&format!("{pascal_str}Rule"), v_ident.span());
        let snake_lit = LitStr::new(&snake_str, v_ident.span());

        entries.push(RegistryEntry {
            pascal_ident: v_ident.clone(),
            snake_ident,
            snake_lit,
            rule_struct_ident,
            aliases,
        });
    }
    Ok(entries)
}

/// Derives `RuleId`, `PuristLintsConfig`, and `default_rules()` from the top-level `RuleDiagnostic` enum.
pub fn derive_rule_registry(input: DeriveInput) -> TokenStream {
    let enum_ident = &input.ident;
    let Data::Enum(data_enum) = &input.data else {
        return Error::new_spanned(
            enum_ident,
            "`RuleRegistry` can only be derived on the top-level `RuleDiagnostic` enum",
        )
        .to_compile_error()
        .into();
    };

    let entries = match parse_registry_variants(enum_ident, &data_enum.variants) {
        Ok(e) => e,
        Err(err) => return err.to_compile_error().into(),
    };

    let pascal_idents: Vec<Ident> = entries.iter().map(|e| e.pascal_ident.clone()).collect();
    let snake_idents: Vec<Ident> = entries.iter().map(|e| e.snake_ident.clone()).collect();
    let snake_lits: Vec<LitStr> = entries.iter().map(|e| e.snake_lit.clone()).collect();
    let rule_struct_idents: Vec<Ident> = entries
        .iter()
        .map(|e| e.rule_struct_ident.clone())
        .collect();
    let alias_groups: Vec<Vec<LitStr>> = entries.iter().map(|e| e.aliases.clone()).collect();

    let from_name_arms = pascal_idents
        .iter()
        .zip(snake_lits.iter())
        .zip(alias_groups.iter())
        .map(|((p_ident, s_lit), aliases)| {
            if aliases.is_empty() {
                quote! { #s_lit => Some(Self::#p_ident), }
            } else {
                quote! { #s_lit #( | #aliases )* => Some(Self::#p_ident), }
            }
        });

    let expanded = quote! {
        /// Canonical identifier for every built-in `purist` lint rule, auto-generated from [`RuleDiagnostic`].
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum RuleId {
            #( #pascal_idents, )*
        }

        impl RuleId {
            /// Slice of all canonical built-in [`RuleId`] variants.
            pub(crate) const ALL: &'static [Self] = &[
                #( Self::#pascal_idents, )*
            ];

            /// Returns the unqualified canonical rule name (e.g. `"error_types"`).
            pub(crate) const fn as_str(self) -> &'static str {
                match self {
                    #( Self::#pascal_idents => #snake_lits, )*
                }
            }

            /// Returns the `purist::`-prefixed rule identifier (e.g. `"purist::error_types"`).
            pub(crate) const fn qualified_name(self) -> &'static str {
                match self {
                    #( Self::#pascal_idents => concat!("purist::", #snake_lits), )*
                }
            }

            /// Parses a rule name (with optional `purist::` or `opinionated::` prefix and legacy aliases) into a [`RuleId`].
            pub(crate) fn from_name(rule_name: &str) -> Option<Self> {
                let stripped = rule_name
                    .strip_prefix("purist::")
                    .or_else(|| rule_name.strip_prefix("opinionated::"))
                    .unwrap_or(rule_name);

                match stripped {
                    #( #from_name_arms )*
                    _ => None,
                }
            }
        }

        impl #enum_ident {
            /// Returns the canonical [`RuleId`] for this diagnostic.
            pub(crate) const fn rule_id(&self) -> RuleId {
                match self {
                    #( Self::#pascal_idents(_) => RuleId::#pascal_idents, )*
                }
            }

            /// Returns the source [`crate::diagnostics::Span`] where this diagnostic was triggered.
            pub const fn span(&self) -> &crate::diagnostics::Span {
                match self {
                    #( Self::#pascal_idents(inner) => inner.span(), )*
                }
            }
        }

        /// Returns a collection of all standard purist static analysis rules, auto-generated from [`RuleDiagnostic`].
        pub(crate) fn default_rules() -> Vec<Box<dyn crate::engine::Rule>> {
            vec![
                #( Box::new(self::#snake_idents::#rule_struct_idents), )*
            ]
        }

        /// Strongly-typed struct holding configuration levels for all known purist lint rules,
        /// auto-generated from [`RuleDiagnostic`].
        #[derive(Debug, Clone, Default, PartialEq, Eq)]
        pub(crate) struct PuristLintsConfig {
            #( pub(crate) #snake_idents: crate::cargo::RuleLevel, )*
        }

        impl PuristLintsConfig {
            pub(crate) fn get(&self, rule_name: &str) -> Option<crate::cargo::RuleLevel> {
                let rule_id = RuleId::from_name(rule_name)?;
                Some(self.level_for_id(rule_id))
            }

            pub(crate) fn set(
                &mut self,
                rule_name: &str,
                level: crate::cargo::RuleLevel,
            ) -> Result<(), crate::cargo::UnrecognizedRule> {
                let rule_id = RuleId::from_name(rule_name).ok_or(crate::cargo::UnrecognizedRule)?;
                *self.level_mut_for_id(rule_id) = level;
                Ok(())
            }

            const fn level_for_id(&self, rule_id: RuleId) -> crate::cargo::RuleLevel {
                match rule_id {
                    #( RuleId::#pascal_idents => self.#snake_idents, )*
                }
            }

            fn level_mut_for_id(&mut self, rule_id: RuleId) -> &mut crate::cargo::RuleLevel {
                match rule_id {
                    #( RuleId::#pascal_idents => &mut self.#snake_idents, )*
                }
            }
        }
    };

    expanded.into()
}
