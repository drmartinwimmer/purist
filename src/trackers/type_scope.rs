//! Scope tracker for enclosing data types, implementations, and variants using a stack.

use super::clap_scope::is_cli_or_command_struct_name;
use syn::Ident;

/// The kind of enclosing data type or declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerKind {
    Struct(String),
    Enum(String),
    Variant {
        enum_name: String,
        variant_name: String,
    },
    Impl(String),
}

/// Scope tracker that observes enclosing structs, enums, variants, and impl blocks using a stack (`Vec`).
#[derive(Debug, Clone, Default)]
pub struct TypeScopeTracker {
    stack: Vec<ContainerKind>,
}

impl TypeScopeTracker {
    /// Creates a new type scope tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Formats a human-readable description of the current container (e.g. `struct 'Foo'` or `enum variant 'Bar::Baz'`).
    pub fn container_description(&self) -> Option<String> {
        self.stack.last().map(|c| match c {
            ContainerKind::Struct(name) => format!("struct '{name}'"),
            ContainerKind::Enum(name) => format!("enum '{name}'"),
            ContainerKind::Variant {
                enum_name,
                variant_name,
            } => format!("enum variant '{enum_name}::{variant_name}'"),
            ContainerKind::Impl(name) => format!("impl '{name}'"),
        })
    }

    /// Returns the name of the current struct if inside a struct definition.
    pub fn current_struct_name(&self) -> Option<&str> {
        self.stack.iter().rev().find_map(|c| match c {
            ContainerKind::Struct(name) => Some(name.as_str()),
            _ => None,
        })
    }

    /// Returns the name of the current enum if inside an enum definition.
    pub fn current_enum_name(&self) -> Option<&str> {
        self.stack.iter().rev().find_map(|c| match c {
            ContainerKind::Enum(name) => Some(name.as_str()),
            _ => None,
        })
    }

    /// Returns the name of the target type if inside an impl block.
    pub fn current_impl_name(&self) -> Option<&str> {
        self.stack.iter().rev().find_map(|c| match c {
            ContainerKind::Impl(name) => Some(name.as_str()),
            _ => None,
        })
    }

    /// Returns true if the current impl block targets a CLI command struct.
    pub fn is_cli_command(&self) -> bool {
        self.current_impl_name()
            .is_some_and(is_cli_or_command_struct_name)
    }

    /// Pushes a struct scope onto the stack.
    pub fn push_struct(&mut self, ident: &Ident) {
        self.stack.push(ContainerKind::Struct(ident.to_string()));
    }

    /// Pushes an enum scope onto the stack.
    pub fn push_enum(&mut self, ident: &Ident) {
        self.stack.push(ContainerKind::Enum(ident.to_string()));
    }

    /// Pushes an enum variant scope onto the stack.
    pub fn push_variant(&mut self, ident: &Ident) {
        let enum_name = self.current_enum_name().unwrap_or("unknown").to_string();
        self.stack.push(ContainerKind::Variant {
            enum_name,
            variant_name: ident.to_string(),
        });
    }

    /// Pushes an impl scope onto the stack.
    pub fn push_impl(&mut self, name: String) {
        self.stack.push(ContainerKind::Impl(name));
    }

    /// Pops the active scope from the stack.
    pub fn pop(&mut self) -> Option<ContainerKind> {
        self.stack.pop()
    }
}
