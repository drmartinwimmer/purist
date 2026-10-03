pub mod engine;
pub mod rules;

use clap::Args;
use code_review_diagnostics::{DiagnosticReport, OutputFormat, render_report};
pub use engine::{LintContext, OpinionatedEngine, Rule};
pub use rules::default_rules;
use std::path::{Path, PathBuf};

/// Error type for opinionated linter execution.
#[derive(Debug, thiserror::Error)]
pub enum OpinionatedError {
    #[error("Target path '{0}' was not found")]
    PathNotFound(PathBuf),

    #[error("I/O error during opinionated lint execution: {0}")]
    Io(#[from] std::io::Error),

    #[error("Opinionated lint violations found ({count} issues)")]
    LintViolationsFound { count: usize },
}

/// Arguments for the opinionated linter subcommand.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct OpinionatedCommand {
    /// Path to source files or crate directory
    #[arg(long)]
    path: Option<PathBuf>,

    /// Output format for reports and diagnostics
    #[arg(long, value_enum)]
    format: Option<OutputFormat>,

    /// Automatically apply fixes where supported (stub)
    #[arg(long)]
    fix: bool,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl OpinionatedCommand {
    /// Creates a new `OpinionatedCommand` instance.
    pub fn new(path: Option<PathBuf>, quiet: bool) -> Self {
        Self {
            path,
            format: None,
            fix: false,
            quiet,
        }
    }

    /// Sets the output format.
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Sets the fix flag.
    pub fn with_fix(mut self, fix: bool) -> Self {
        self.fix = fix;
        self
    }

    /// Returns the target path, if specified.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the specified output format, if any.
    pub fn format(&self) -> Option<OutputFormat> {
        self.format
    }

    /// Returns whether automated fixing is requested.
    pub fn is_fix(&self) -> bool {
        self.fix
    }

    /// Returns whether logging output is suppressed.
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Executes the opinionated rules against the target path and returns the report.
    pub fn execute(&self) -> Result<DiagnosticReport, OpinionatedError> {
        let target_path = self.path.as_deref().unwrap_or_else(|| Path::new("."));

        if !target_path.exists() {
            return Err(OpinionatedError::PathNotFound(target_path.to_path_buf()));
        }

        let engine = OpinionatedEngine::new();
        let report = engine.check_path(target_path)?;
        Ok(report)
    }

    /// Runs the opinionated static analysis checks and renders diagnostics.
    pub fn run(&self) -> Result<(), OpinionatedError> {
        let report = self.execute()?;
        let format = self.format.unwrap_or(OutputFormat::Console);

        render_report(&report, format, &mut std::io::stdout())?;

        if !report.is_empty() {
            Err(OpinionatedError::LintViolationsFound {
                count: report.diagnostics.len(),
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::fs;

    #[googletest::test]
    fn run_opinionated_command_on_missing_path_returns_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let missing = PathBuf::from("does_not_exist_12345.rs");
        let cmd = OpinionatedCommand::new(Some(missing.clone()), true);
        match cmd.execute() {
            Err(OpinionatedError::PathNotFound(p)) => {
                assert_that!(p, eq(&missing));
            }
            other => return Err(format!("Expected PathNotFound, got {other:?}").into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn run_opinionated_command_on_clean_file_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_clean_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let file_path = temp_dir.join("clean.rs");
        fs::write(&file_path, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n")?;

        let cmd = OpinionatedCommand::new(Some(file_path), true);
        let report = cmd.execute()?;

        let _ = fs::remove_dir_all(&temp_dir);

        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn run_opinionated_command_detects_violations() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_violations_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let file_path = temp_dir.join("bad.rs");
        fs::write(
            &file_path,
            "pub fn fail() -> Result<(), String> { Err(\"bad\".to_string()) }\n",
        )?;

        let cmd = OpinionatedCommand::new(Some(file_path), true);
        let result = cmd.run();

        let _ = fs::remove_dir_all(&temp_dir);

        match result {
            Err(OpinionatedError::LintViolationsFound { count }) => {
                assert_that!(count, eq(1));
            }
            other => return Err(format!("Expected LintViolationsFound, got {other:?}").into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn parse_opinionated_command_with_options() -> Result<(), Box<dyn std::error::Error>> {
        let cmd = OpinionatedCommand::new(Some(PathBuf::from("src")), false)
            .with_format(OutputFormat::Json)
            .with_fix(true);

        assert_that!(cmd.path(), eq(Some(Path::new("src"))));
        assert_that!(cmd.format(), eq(Some(OutputFormat::Json)));
        assert_that!(cmd.is_fix(), is_true());
        assert_that!(cmd.is_quiet(), is_false());
        Ok(())
    }
}
