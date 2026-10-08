//! Scope tracker for Clap command and argument models during AST traversal using a stack.

use super::guard::run_with_scope;
use crate::rules::common::{derives_any, derives_trait};
use syn::{Attribute, Field, ItemStruct};

/// Information describing the currently active Clap struct scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClapStructInfo {
    pub name: String,
    pub is_command: bool,
    pub derives_args: bool,
    pub derives_parser: bool,
}

/// Scope tracker that observes Clap CLI definitions during AST traversal using a stack (`Vec`).
#[derive(Debug, Clone, Default)]
pub struct ClapScope {
    stack: Vec<Option<ClapStructInfo>>,
    cli_impl_stack: Vec<bool>,
    command_runner_stack: Vec<bool>,
}

impl ClapScope {
    /// Creates a new Clap scope tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if currently traversing within a Clap struct (`Args` or `Parser`).
    pub fn is_in_clap_struct(&self) -> bool {
        self.current_struct().is_some()
    }

    /// Returns information about the current Clap struct, if any.
    pub fn current_struct(&self) -> Option<&ClapStructInfo> {
        self.stack.iter().rev().find_map(|s| s.as_ref())
    }

    /// Returns the name of the current Clap struct, if any.
    pub fn current_struct_name(&self) -> Option<&str> {
        self.current_struct().map(|s| s.name.as_str())
    }

    /// Returns true if the current Clap struct represents an executable command.
    pub fn is_current_command(&self) -> bool {
        self.current_struct().is_some_and(|s| s.is_command)
    }

    /// Pushes a struct scope onto the stack.
    pub fn push_struct(&mut self, item_struct: &ItemStruct) {
        if derives_clap(&item_struct.attrs) {
            let name = item_struct.ident.to_string();
            let is_command = is_command_struct_name(&name);
            let derives_args = derives_trait(&item_struct.attrs, "Args");
            let derives_parser = derives_trait(&item_struct.attrs, "Parser");
            self.stack.push(Some(ClapStructInfo {
                name,
                is_command,
                derives_args,
                derives_parser,
            }));
        } else {
            self.stack.push(None);
        }
    }

    /// Pops the active struct scope from the stack.
    pub fn pop(&mut self) -> Option<Option<ClapStructInfo>> {
        self.stack.pop()
    }

    /// Executes a closure within an entered struct scope.
    pub fn with_struct<R>(
        &mut self,
        item_struct: &ItemStruct,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        run_with_scope(
            self,
            |t| t.push_struct(item_struct),
            |t| {
                t.pop();
            },
            f,
        )
    }

    /// Returns true if currently traversing within a CLI implementation block.
    pub fn is_in_cli_impl(&self) -> bool {
        self.cli_impl_stack.last().copied().unwrap_or(false)
    }

    /// Returns true if currently traversing within a CLI command runner method (`run`, `execute`).
    pub fn is_in_command_runner(&self) -> bool {
        self.command_runner_stack.last().copied().unwrap_or(false)
    }

    /// Pushes the CLI implementation block status onto the stack.
    pub fn push_cli_impl(&mut self, is_cli: bool) {
        self.cli_impl_stack.push(is_cli);
    }

    /// Pops the active CLI implementation block status from the stack.
    pub fn pop_cli_impl(&mut self) -> Option<bool> {
        self.cli_impl_stack.pop()
    }

    /// Pushes the CLI command runner execution status onto the stack.
    pub fn push_command_runner(&mut self, is_runner: bool) {
        self.command_runner_stack.push(is_runner);
    }

    /// Pops the active CLI command runner execution status from the stack.
    pub fn pop_command_runner(&mut self) -> Option<bool> {
        self.command_runner_stack.pop()
    }

    /// Executes a closure within a CLI implementation block scope.
    pub fn with_cli_impl<R>(&mut self, is_cli: bool, f: impl FnOnce(&mut Self) -> R) -> R {
        run_with_scope(
            self,
            |t| t.push_cli_impl(is_cli),
            |t| {
                t.pop_cli_impl();
            },
            f,
        )
    }

    /// Executes a closure within a CLI command runner scope.
    pub fn with_command_runner<R>(&mut self, is_runner: bool, f: impl FnOnce(&mut Self) -> R) -> R {
        run_with_scope(
            self,
            |t| t.push_command_runner(is_runner),
            |t| {
                t.pop_command_runner();
            },
            f,
        )
    }
}

/// Trait for visitor types that hold a [`ClapScope`], providing scoped closure methods.
pub trait WithClapScope {
    /// Returns a mutable reference to the underlying Clap scope.
    fn clap_scope_mut(&mut self) -> &mut ClapScope;

    /// Runs a closure within an entered Clap struct scope.
    fn with_clap_struct<R>(&mut self, item_struct: &ItemStruct, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.clap_scope_mut().push_struct(item_struct),
            |v| {
                v.clap_scope_mut().pop();
            },
            f,
        )
    }

    /// Runs a closure within a CLI implementation block scope.
    fn with_cli_impl<R>(&mut self, is_cli: bool, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.clap_scope_mut().push_cli_impl(is_cli),
            |v| {
                v.clap_scope_mut().pop_cli_impl();
            },
            f,
        )
    }

    /// Runs a closure within a CLI command runner scope.
    fn with_command_runner<R>(&mut self, is_runner: bool, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.clap_scope_mut().push_command_runner(is_runner),
            |v| {
                v.clap_scope_mut().pop_command_runner();
            },
            f,
        )
    }
}

impl WithClapScope for ClapScope {
    fn clap_scope_mut(&mut self) -> &mut ClapScope {
        self
    }
}

/// Checks whether an attribute list derives `Args` or `Parser`.
pub fn derives_clap(attrs: &[Attribute]) -> bool {
    derives_any(attrs, &["Args", "Parser"])
}

/// Checks whether a struct field is annotated with `#[command(flatten)]` or `#[arg(flatten)]`.
pub fn is_flattened_field(field: &Field) -> bool {
    field.attrs.iter().any(|attr| {
        if !attr.path().is_ident("command") && !attr.path().is_ident("arg") {
            return false;
        }
        let mut is_flatten = false;
        let _result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("flatten") {
                is_flatten = true;
            }
            Ok(())
        });
        is_flatten
    })
}

/// Returns true if the struct name suggests a CLI command.
pub fn is_command_struct_name(name: &str) -> bool {
    name.ends_with("Command")
        || name.ends_with("Args")
        || name.ends_with("Subcommand")
        || name.ends_with("Subcommands")
}

/// Returns true if the type name matches CLI top-level or command conventions.
pub fn is_cli_or_command_struct_name(name: &str) -> bool {
    name == "Cli"
        || name == "App"
        || name == "Commands"
        || name.ends_with("Cli")
        || is_command_struct_name(name)
}

/// Returns true if the function name suggests a CLI command execution method.
pub fn is_command_execution_fn_name(name: &str) -> bool {
    matches!(name, "run" | "execute" | "run_command" | "run_with_format")
}
