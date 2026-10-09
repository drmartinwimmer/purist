//! Implementation of scope trait derive logic.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{Attribute, Data, DeriveInput, Error, Field, Fields, Ident, Meta, Type};

/// Checks if an attribute explicitly targets the scope (via `#[scope(tag)]` or `#[specific_attr]`).
fn has_scope_tag(attr: &Attribute, tag: &str, specific_attr: &str) -> bool {
    if attr.path().is_ident(specific_attr) {
        return true;
    }
    if attr.path().is_ident("scope")
        && let Meta::List(meta_list) = &attr.meta
    {
        let mut found = false;
        let _ = meta_list.parse_nested_meta(|nested| {
            if nested.path.is_ident(tag) {
                found = true;
            }
            Ok(())
        });
        return found;
    }
    false
}

/// Checks if an attribute is a bare `#[scope]`.
fn has_bare_scope(attr: &Attribute) -> bool {
    attr.path().is_ident("scope") && matches!(attr.meta, Meta::Path(_))
}

/// Checks if a type matches any of the expected type names.
fn is_matching_type(ty: &Type, expected_type_names: &[&str]) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let Some(last_segment) = type_path.path.segments.last() else {
        return false;
    };
    expected_type_names.contains(&last_segment.ident.to_string().as_str())
}

/// Finds the single field matching the scope criteria, ensuring no ambiguity.
fn find_scope_field<'a>(
    struct_name: &Ident,
    fields: &'a Punctuated<Field, Comma>,
    tag: &str,
    specific_attr: &str,
    expected_field_names: &[&str],
    expected_type_names: &[&str],
) -> syn::Result<Option<&'a Field>> {
    // 1. Explicit tag attribute (e.g. #[test_scope] or #[scope(test)])
    let tagged: Vec<&Field> = fields
        .iter()
        .filter(|f| f.attrs.iter().any(|a| has_scope_tag(a, tag, specific_attr)))
        .collect();

    if tagged.len() > 1 {
        return Err(Error::new_spanned(
            struct_name,
            format!("Multiple fields match attribute `#[{specific_attr}]` or `#[scope({tag})]`"),
        ));
    }
    if let Some(&f) = tagged.first() {
        return Ok(Some(f));
    }

    // 2. Bare #[scope] attribute with matching type
    let bare_scope: Vec<&Field> = fields
        .iter()
        .filter(|f| {
            f.attrs.iter().any(has_bare_scope) && is_matching_type(&f.ty, expected_type_names)
        })
        .collect();

    if bare_scope.len() > 1 {
        return Err(Error::new_spanned(
            struct_name,
            format!(
                "Multiple fields of type `{}` have `#[scope]`. Disambiguate with `#[{specific_attr}]` or `#[scope({tag})]`",
                expected_type_names[0]
            ),
        ));
    }
    if let Some(&f) = bare_scope.first() {
        return Ok(Some(f));
    }

    // 3. Field name match
    let name_matched: Vec<&Field> = fields
        .iter()
        .filter(|f| {
            f.ident
                .as_ref()
                .is_some_and(|id| expected_field_names.contains(&id.to_string().as_str()))
        })
        .collect();

    if name_matched.len() > 1 {
        return Err(Error::new_spanned(
            struct_name,
            format!("Multiple fields match name `{}`", expected_field_names[0]),
        ));
    }
    if let Some(&f) = name_matched.first() {
        return Ok(Some(f));
    }

    // 4. Field type match
    let type_matched: Vec<&Field> = fields
        .iter()
        .filter(|f| is_matching_type(&f.ty, expected_type_names))
        .collect();

    if type_matched.len() > 1 {
        return Err(Error::new_spanned(
            struct_name,
            format!(
                "Multiple fields match type `{}`. Disambiguate with `#[{specific_attr}]` or `#[scope({tag})]`",
                expected_type_names[0]
            ),
        ));
    }
    if let Some(&f) = type_matched.first() {
        return Ok(Some(f));
    }

    Ok(None)
}

/// Derives a scope trait for the given input struct.
#[expect(
    clippy::too_many_arguments,
    reason = "Procedural macro derive parameterization"
)]
pub fn derive_scope(
    input: DeriveInput,
    trait_name: &str,
    method_name: &str,
    scope_type: &str,
    tag: &str,
    specific_attr: &str,
    expected_field_names: &[&str],
    expected_type_names: &[&str],
) -> TokenStream {
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let Data::Struct(data_struct) = &input.data else {
        return Error::new_spanned(
            struct_name,
            format!("`{trait_name}` can only be derived for structs"),
        )
        .to_compile_error()
        .into();
    };

    let Fields::Named(fields_named) = &data_struct.fields else {
        return Error::new_spanned(
            struct_name,
            format!("`{trait_name}` can only be derived for structs with named fields"),
        )
        .to_compile_error()
        .into();
    };

    let field = match find_scope_field(
        struct_name,
        &fields_named.named,
        tag,
        specific_attr,
        expected_field_names,
        expected_type_names,
    ) {
        Ok(Some(f)) => f,
        Ok(None) => {
            return Error::new_spanned(
                struct_name,
                format!(
                    "`{trait_name}` requires a field named `{}` or of type `{}`, or annotated with `#[{specific_attr}]` / `#[scope({tag})]`",
                    expected_field_names[0], scope_type
                ),
            )
            .to_compile_error()
            .into();
        }
        Err(err) => {
            return err.to_compile_error().into();
        }
    };

    let field_ident = &field.ident;
    let trait_ident = Ident::new(trait_name, Span::call_site());
    let method_ident = Ident::new(method_name, Span::call_site());
    let scope_type_ident = Ident::new(scope_type, Span::call_site());

    let expanded = quote! {
        impl #impl_generics ::purist::scopes::#trait_ident for #struct_name #ty_generics #where_clause {
            fn #method_ident(&mut self) -> &mut ::purist::scopes::#scope_type_ident {
                &mut self.#field_ident
            }
        }
    };

    expanded.into()
}

/// Derives the `WithBlockScope` trait for the given input struct.
pub fn derive_block_scope(input: DeriveInput) -> TokenStream {
    let struct_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let Data::Struct(data_struct) = &input.data else {
        return Error::new_spanned(
            struct_name,
            "`WithBlockScope` can only be derived for structs",
        )
        .to_compile_error()
        .into();
    };

    let Fields::Named(fields_named) = &data_struct.fields else {
        return Error::new_spanned(
            struct_name,
            "`WithBlockScope` can only be derived for structs with named fields",
        )
        .to_compile_error()
        .into();
    };

    let field = match find_scope_field(
        struct_name,
        &fields_named.named,
        "block",
        "block_scope",
        &["block_scope"],
        &["BlockScope"],
    ) {
        Ok(Some(f)) => f,
        Ok(None) => {
            return Error::new_spanned(
                struct_name,
                "`WithBlockScope` requires a field named `block_scope` or of type `BlockScope<K, V>`, or annotated with `#[block_scope]` / `#[scope(block)]`",
            )
            .to_compile_error()
            .into();
        }
        Err(err) => {
            return err.to_compile_error().into();
        }
    };

    let (k_ty, v_ty) = match &field.ty {
        Type::Path(type_path) => {
            let last = type_path.path.segments.last().unwrap();
            if let syn::PathArguments::AngleBracketed(args) = &last.arguments
                && args.args.len() == 2
                && let syn::GenericArgument::Type(ref k) = args.args[0]
                && let syn::GenericArgument::Type(ref v) = args.args[1]
            {
                (k.clone(), v.clone())
            } else {
                return Error::new_spanned(
                    &field.ty,
                    "`BlockScope` requires two generic type arguments: `BlockScope<K, V>`",
                )
                .to_compile_error()
                .into();
            }
        }
        _ => {
            return Error::new_spanned(&field.ty, "Expected `BlockScope<K, V>` type")
                .to_compile_error()
                .into();
        }
    };

    let field_ident = &field.ident;

    let expanded = quote! {
        impl #impl_generics ::purist::scopes::WithBlockScope<#k_ty, #v_ty> for #struct_name #ty_generics #where_clause {
            fn block_scope_mut(&mut self) -> &mut ::purist::scopes::BlockScope<#k_ty, #v_ty> {
                &mut self.#field_ident
            }
        }
    };

    expanded.into()
}
