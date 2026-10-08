//! Scope tracker for Clap command and argument models during AST traversal using a stack.

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
pub struct ClapScopeTracker {
    stack: Vec<Option<ClapStructInfo>>,
}

impl ClapScopeTracker {
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
    name == "Cli" || name == "App" || is_command_struct_name(name)
}

/// Returns true if the function name suggests a CLI command execution method.
pub fn is_command_execution_fn_name(name: &str) -> bool {
    matches!(name, "run" | "execute" | "run_command")
}
