//! Scope tracker for enclosing data types, implementations, and variants using a stack.

use super::guard::run_with_scope;
use syn::Ident;

/// The kind of enclosing data type or declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContainerKind {
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
pub(crate) struct TypeScope {
    stack: Vec<ContainerKind>,
    drop_impl_stack: Vec<bool>,
}

impl TypeScope {
    /// Creates a new type scope tracker.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Formats a human-readable description of the current container (e.g. `struct 'Foo'` or `enum variant 'Bar::Baz'`).
    pub(crate) fn container_description(&self) -> Option<String> {
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

    /// Returns the name of the current enum if inside an enum definition.
    pub(crate) fn current_enum_name(&self) -> Option<&str> {
        self.stack.iter().rev().find_map(|c| match c {
            ContainerKind::Enum(name) => Some(name.as_str()),
            _ => None,
        })
    }

    /// Returns the name of the target type if inside an impl block.
    pub(crate) fn current_impl_name(&self) -> Option<&str> {
        self.stack.iter().rev().find_map(|c| match c {
            ContainerKind::Impl(name) => Some(name.as_str()),
            _ => None,
        })
    }

    /// Pushes a struct scope onto the stack.
    pub(crate) fn push_struct(&mut self, ident: &Ident) {
        self.stack.push(ContainerKind::Struct(ident.to_string()));
    }

    /// Pushes an enum scope onto the stack.
    pub(crate) fn push_enum(&mut self, ident: &Ident) {
        self.stack.push(ContainerKind::Enum(ident.to_string()));
    }

    /// Pushes an enum variant scope onto the stack.
    pub(crate) fn push_variant(&mut self, ident: &Ident) {
        let enum_name = self.current_enum_name().unwrap_or("unknown").to_string();
        self.stack.push(ContainerKind::Variant {
            enum_name,
            variant_name: ident.to_string(),
        });
    }

    /// Pushes an impl scope onto the stack.
    pub(crate) fn push_impl(&mut self, name: String) {
        self.stack.push(ContainerKind::Impl(name));
    }

    /// Pops the active scope from the stack.
    pub(crate) fn pop(&mut self) -> Option<ContainerKind> {
        self.stack.pop()
    }

    /// Returns true if currently traversing inside a `Drop` trait implementation.
    pub(crate) fn is_in_drop_impl(&self) -> bool {
        self.drop_impl_stack.last().copied().unwrap_or(false)
    }

    /// Pushes the Drop implementation block status onto the stack.
    pub(crate) fn push_drop_impl(&mut self, in_drop: bool) {
        self.drop_impl_stack.push(in_drop);
    }

    /// Pops the active Drop implementation block status from the stack.
    pub(crate) fn pop_drop_impl(&mut self) -> Option<bool> {
        self.drop_impl_stack.pop()
    }
}

/// Trait for visitor types that hold a [`TypeScope`], providing scoped closure methods.
pub(crate) trait WithTypeScope {
    /// Returns a mutable reference to the underlying type scope.
    fn type_scope_mut(&mut self) -> &mut TypeScope;

    /// Runs a closure within an entered struct scope.
    fn with_type_struct<R>(&mut self, ident: &Ident, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.type_scope_mut().push_struct(ident),
            |v| {
                v.type_scope_mut().pop();
            },
            f,
        )
    }

    /// Runs a closure within an entered enum scope.
    fn with_type_enum<R>(&mut self, ident: &Ident, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.type_scope_mut().push_enum(ident),
            |v| {
                v.type_scope_mut().pop();
            },
            f,
        )
    }

    /// Runs a closure within an entered enum variant scope.
    fn with_type_variant<R>(&mut self, ident: &Ident, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.type_scope_mut().push_variant(ident),
            |v| {
                v.type_scope_mut().pop();
            },
            f,
        )
    }

    /// Runs a closure within an entered impl scope.
    fn with_type_impl<R>(&mut self, name: String, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.type_scope_mut().push_impl(name),
            |v| {
                v.type_scope_mut().pop();
            },
            f,
        )
    }

    /// Runs a closure within a Drop implementation scope.
    fn with_drop_impl<R>(&mut self, in_drop: bool, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.type_scope_mut().push_drop_impl(in_drop),
            |v| {
                v.type_scope_mut().pop_drop_impl();
            },
            f,
        )
    }
}

impl WithTypeScope for TypeScope {
    fn type_scope_mut(&mut self) -> &mut TypeScope {
        self
    }
}
