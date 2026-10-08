//! Procedural derive macros for purist AST visitor scope traits.

mod derive;

use derive::derive_scope;
use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Derives the `WithTestScope` trait for an AST visitor struct.
#[proc_macro_derive(WithTestScope, attributes(scope, test_scope))]
pub fn derive_with_test_scope(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_scope(
        input,
        "WithTestScope",
        "test_scope_mut",
        "TestScope",
        "test",
        "test_scope",
        &["test_scope"],
        &["TestScope"],
    )
}

/// Derives the `WithClapScope` trait for an AST visitor struct.
#[proc_macro_derive(WithClapScope, attributes(scope, clap_scope))]
pub fn derive_with_clap_scope(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_scope(
        input,
        "WithClapScope",
        "clap_scope_mut",
        "ClapScope",
        "clap",
        "clap_scope",
        &["clap_scope"],
        &["ClapScope"],
    )
}

/// Derives the `WithTypeScope` trait for an AST visitor struct.
#[proc_macro_derive(WithTypeScope, attributes(scope, type_scope))]
pub fn derive_with_type_scope(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_scope(
        input,
        "WithTypeScope",
        "type_scope_mut",
        "TypeScope",
        "type",
        "type_scope",
        &["type_scope"],
        &["TypeScope"],
    )
}

/// Derives the `WithDepthScope` trait for an AST visitor struct.
#[proc_macro_derive(WithDepthScope, attributes(scope, depth_scope))]
pub fn derive_with_depth_scope(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive_scope(
        input,
        "WithDepthScope",
        "depth_scope_mut",
        "DepthScope",
        "depth",
        "depth_scope",
        &["depth_scope", "depth"],
        &["DepthScope"],
    )
}
