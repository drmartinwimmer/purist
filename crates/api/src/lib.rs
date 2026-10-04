use clap::Args;
use std::path::{Path, PathBuf};

/// Error type for API drift inspection execution.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("I/O error during API inspection: {0}")]
    Io(#[from] std::io::Error),
}

/// Arguments for the API manifest and drift inspection subcommand.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct ApiCommand {
    /// Path to Cargo.toml or workspace root
    #[arg(long)]
    manifest_path: Option<PathBuf>,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl ApiCommand {
    /// Creates a new `ApiCommand` instance.
    pub fn new(manifest_path: Option<PathBuf>, quiet: bool) -> Self {
        Self {
            manifest_path,
            quiet,
        }
    }

    /// Returns the target manifest path, if specified.
    pub fn manifest_path(&self) -> Option<&Path> {
        self.manifest_path.as_deref()
    }

    /// Returns whether logging output is suppressed.
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Runs the API manifest inspection checks.
    pub fn run(self) -> Result<(), ApiError> {
        if !self.quiet {
            eprintln!("Notice: api drift detector is scheduled for future milestones.");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn run_api_command_succeeds() -> googletest::Result<()> {
        let cmd = ApiCommand::new(None, true);
        assert_that!(cmd.run(), ok(anything()));
        Ok(())
    }
}
