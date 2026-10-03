use clap::Args;
use std::path::{Path, PathBuf};

/// Error type for check aggregator execution.
#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("I/O error during check execution: {0}")]
    Io(#[from] std::io::Error),
}

/// Arguments for the check aggregator subcommand.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckCommand {
    /// Path to target workspace or crate directory
    #[arg(long)]
    path: Option<PathBuf>,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl CheckCommand {
    /// Creates a new `CheckCommand` instance.
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

    /// Runs the check aggregator.
    pub fn run(&self) -> Result<(), CheckError> {
        if !self.quiet {
            eprintln!("Notice: check aggregator is scheduled for future milestones.");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn run_check_command_succeeds() -> googletest::Result<()> {
        let cmd = CheckCommand::new(None, true);
        assert_that!(cmd.run(), ok(anything()));
        Ok(())
    }
}
