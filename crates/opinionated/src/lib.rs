use clap::Args;
use std::path::{Path, PathBuf};

/// Error type for opinionated linter execution.
#[derive(Debug, thiserror::Error)]
pub enum OpinionatedError {
    #[error("I/O error during opinionated lint execution: {0}")]
    Io(#[from] std::io::Error),
}

/// Arguments for the opinionated linter subcommand.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct OpinionatedCommand {
    /// Path to source files or crate directory
    #[arg(long)]
    path: Option<PathBuf>,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl OpinionatedCommand {
    /// Creates a new `OpinionatedCommand` instance.
    pub fn new(path: Option<PathBuf>, quiet: bool) -> Self {
        Self { path, quiet }
    }

    /// Returns the target path, if specified.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns whether logging output is suppressed.
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Runs the opinionated static analysis checks.
    pub fn run(&self) -> Result<(), OpinionatedError> {
        if !self.quiet {
            eprintln!("Notice: opinionated linter is scheduled for Milestone 2.");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn run_opinionated_command_succeeds() -> googletest::Result<()> {
        let cmd = OpinionatedCommand::new(None, true);
        assert_that!(cmd.run(), ok(anything()));
        Ok(())
    }
}
