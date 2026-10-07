//! Scope tracker for enclosing data types, implementations, and variants.

use super::clap_scope::is_cli_or_command_struct_name;
use super::guard::RefScopeGuard;
use std::cell::RefCell;
use std::rc::Rc;
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

/// Snapshot state of the enclosing type scope.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeScopeState {
    pub current_container: Option<ContainerKind>,
    pub current_enum: Option<String>,
}

/// Scope tracker that observes enclosing structs, enums, variants, and impl blocks.
#[derive(Debug, Clone, Default)]
pub struct TypeScopeTracker {
    state: Rc<RefCell<TypeScopeState>>,
}

impl TypeScopeTracker {
    /// Creates a new type scope tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Formats a human-readable description of the current container (e.g. `struct 'Foo'` or `enum variant 'Bar::Baz'`).
    pub fn container_description(&self) -> Option<String> {
        let state = self.state.borrow();
        state.current_container.as_ref().map(|c| match c {
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
    pub fn current_struct_name(&self) -> Option<String> {
        self.state
            .borrow()
            .current_container
            .as_ref()
            .and_then(|c| match c {
                ContainerKind::Struct(name) => Some(name.clone()),
                _ => None,
            })
    }

    /// Returns the name of the target type if inside an impl block.
    pub fn current_impl_name(&self) -> Option<String> {
        self.state
            .borrow()
            .current_container
            .as_ref()
            .and_then(|c| match c {
                ContainerKind::Impl(name) => Some(name.clone()),
                _ => None,
            })
    }

    /// Returns true if the current impl block targets a CLI command struct.
    pub fn is_cli_command(&self) -> bool {
        self.current_impl_name()
            .is_some_and(|name| is_cli_or_command_struct_name(&name))
    }

    /// Enters a struct scope and returns an RAII guard that restores previous scope on drop.
    pub fn enter_struct(&self, ident: &Ident) -> RefScopeGuard<TypeScopeState> {
        let mut state = self.state.borrow_mut();
        let prev = state.clone();
        state.current_container = Some(ContainerKind::Struct(ident.to_string()));
        drop(state);
        RefScopeGuard::new(Rc::clone(&self.state), prev)
    }

    /// Enters an enum scope and returns an RAII guard that restores previous scope on drop.
    pub fn enter_enum(&self, ident: &Ident) -> RefScopeGuard<TypeScopeState> {
        let mut state = self.state.borrow_mut();
        let prev = state.clone();
        let enum_name = ident.to_string();
        state.current_container = Some(ContainerKind::Enum(enum_name.clone()));
        state.current_enum = Some(enum_name);
        drop(state);
        RefScopeGuard::new(Rc::clone(&self.state), prev)
    }

    /// Enters an enum variant scope and returns an RAII guard that restores previous scope on drop.
    pub fn enter_variant(&self, ident: &Ident) -> RefScopeGuard<TypeScopeState> {
        let mut state = self.state.borrow_mut();
        let prev = state.clone();
        let enum_name = state
            .current_enum
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        state.current_container = Some(ContainerKind::Variant {
            enum_name,
            variant_name: ident.to_string(),
        });
        drop(state);
        RefScopeGuard::new(Rc::clone(&self.state), prev)
    }

    /// Enters an impl scope and returns an RAII guard that restores previous scope on drop.
    pub fn enter_impl(&self, name: String) -> RefScopeGuard<TypeScopeState> {
        let mut state = self.state.borrow_mut();
        let prev = state.clone();
        state.current_container = Some(ContainerKind::Impl(name));
        drop(state);
        RefScopeGuard::new(Rc::clone(&self.state), prev)
    }
}
