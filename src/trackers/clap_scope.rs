//! Scope tracker for Clap command and argument models during AST traversal.

use super::guard::RefScopeGuard;
use crate::rules::common::{derives_any, derives_trait};
use std::cell::RefCell;
use std::rc::Rc;
use syn::{Attribute, Field, ItemStruct};

/// Information describing the currently active Clap struct scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClapStructInfo {
    pub name: String,
    pub is_command: bool,
    pub derives_args: bool,
    pub derives_parser: bool,
}

/// Snapshot state of the Clap scope.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClapScopeState {
    pub current_struct: Option<ClapStructInfo>,
}

/// Scope tracker that observes Clap CLI definitions during AST traversal.
#[derive(Debug, Clone, Default)]
pub struct ClapScopeTracker {
    state: Rc<RefCell<ClapScopeState>>,
}

impl ClapScopeTracker {
    /// Creates a new Clap scope tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if currently traversing within a Clap struct (`Args` or `Parser`).
    pub fn is_in_clap_struct(&self) -> bool {
        self.state.borrow().current_struct.is_some()
    }

    /// Returns information about the current Clap struct, if any.
    pub fn current_struct(&self) -> Option<ClapStructInfo> {
        self.state.borrow().current_struct.clone()
    }

    /// Returns the name of the current Clap struct, if any.
    pub fn current_struct_name(&self) -> Option<String> {
        self.state
            .borrow()
            .current_struct
            .as_ref()
            .map(|s| s.name.clone())
    }

    /// Returns true if the current Clap struct represents an executable command.
    pub fn is_current_command(&self) -> bool {
        self.state
            .borrow()
            .current_struct
            .as_ref()
            .is_some_and(|s| s.is_command)
    }

    /// Enters a struct scope and returns an RAII guard that restores previous scope on drop.
    pub fn enter_struct(&self, item_struct: &ItemStruct) -> RefScopeGuard<ClapScopeState> {
        let is_clap = derives_clap(&item_struct.attrs);
        let next_struct = if is_clap {
            let name = item_struct.ident.to_string();
            let is_command = is_command_struct_name(&name);
            let derives_args = derives_trait(&item_struct.attrs, "Args");
            let derives_parser = derives_trait(&item_struct.attrs, "Parser");
            Some(ClapStructInfo {
                name,
                is_command,
                derives_args,
                derives_parser,
            })
        } else {
            None
        };

        let mut state = self.state.borrow_mut();
        let prev = state.clone();
        state.current_struct = next_struct;
        drop(state);

        RefScopeGuard::new(Rc::clone(&self.state), prev)
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

/// Returns true if the struct represents an executable command rather than shared configuration or options.
pub fn is_command_struct_name(name: &str) -> bool {
    if name.ends_with("Options")
        || name.ends_with("Opts")
        || name.ends_with("Config")
        || name.ends_with("Flags")
        || name.starts_with("Common")
        || name == "Cli"
        || name == "Args"
    {
        return false;
    }
    name.ends_with("Command") || name.ends_with("Args")
}

/// Returns true if the type name matches CLI command conventions (`Command`, `Cli`, `Commands`).
pub fn is_cli_or_command_struct_name(name: &str) -> bool {
    name.ends_with("Command") || name.ends_with("Cli") || name == "Cli" || name == "Commands"
}

/// Returns true if the function name represents a command execution runner.
pub fn is_command_execution_fn_name(name: &str) -> bool {
    name == "run" || name == "run_with_format" || name == "execute"
}
