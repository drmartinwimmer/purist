pub mod cargo;
pub mod diagnostics;
pub mod discovery;
pub mod engine;
pub mod reporter;
pub mod rule_config;
pub mod rules;
pub mod trackers;

pub use cargo::{LintConfig, OpinionatedLintsConfig, PuristLintsConfig, RuleLevel};
use clap::Args;
pub use diagnostics::{Diagnostic, DiagnosticReport, ReportSummary, Severity, Span};
pub use engine::{LintContext, OpinionatedEngine, PuristEngine, Rule};
pub use reporter::{OutputFormat, render_report, render_report_with_options};
pub use rules::default_rules;
use std::path::{Path, PathBuf};
pub use trackers::{
    ClapScopeTracker, RefScopeGuard, ScopeGuard, TestScopeTracker, TypeScopeTracker,
};

/// Error type for purist linter execution.
#[derive(Debug, thiserror::Error)]
pub enum PuristError {
    #[error("Target path '{0}' was not found")]
    PathNotFound(PathBuf),

    #[error("I/O error during purist lint execution: {0}")]
    Io(#[from] std::io::Error),

    #[error("Purist lint violations found ({count} issues)")]
    LintViolationsFound { count: usize },
}

/// Backwards compatibility alias for `PuristError`.
pub type OpinionatedError = PuristError;

/// Arguments for the purist linter subcommand.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct PuristCommand {
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

/// Backwards compatibility alias for `PuristCommand`.
pub type OpinionatedCommand = PuristCommand;

impl PuristCommand {
    /// Creates a new `PuristCommand` instance.
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

    /// Executes the purist rules against the target path and returns the report.
    pub fn execute(self) -> Result<DiagnosticReport, PuristError> {
        let target_path = self.path.as_deref().unwrap_or_else(|| Path::new("."));

        if !target_path.exists() {
            return Err(PuristError::PathNotFound(target_path.to_path_buf()));
        }

        let engine = PuristEngine::new();
        let report = engine.check_path(target_path)?;
        Ok(report)
    }

    /// Runs the purist static analysis checks and renders diagnostics.
    pub fn run(self) -> Result<(), PuristError> {
        let format = self.format.unwrap_or(OutputFormat::Console);
        let report = self.execute()?;

        render_report(&report, format, &mut std::io::stdout())?;

        if !report.is_empty() {
            Err(PuristError::LintViolationsFound {
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
    fn run_purist_command_on_missing_path_returns_error() -> Result<(), Box<dyn std::error::Error>>
    {
        let missing = PathBuf::from("does_not_exist_12345.rs");
        let cmd = PuristCommand::new(Some(missing.clone()), true);
        match cmd.execute() {
            Err(PuristError::PathNotFound(p)) => {
                assert_that!(p, eq(&missing));
            }
            other => return Err(format!("Expected PathNotFound, got {other:?}").into()),
        }
        Ok(())
    }

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.0));
        }
    }

    #[googletest::test]
    fn run_purist_command_on_clean_file_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_clean_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("clean.rs");
        fs::write(&file_path, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n")?;

        let cmd = PuristCommand::new(Some(file_path), true);
        let report = cmd.execute()?;

        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn run_purist_command_detects_violations() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_violations_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("bad.rs");
        fs::write(
            &file_path,
            "pub fn fail() -> Result<(), String> { Err(\"bad\".to_string()) }\n",
        )?;

        let cmd = PuristCommand::new(Some(file_path), true);
        let result = cmd.run();

        match result {
            Err(PuristError::LintViolationsFound { count }) => {
                assert_that!(count, eq(1));
            }
            other => return Err(format!("Expected LintViolationsFound, got {other:?}").into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn parse_purist_command_with_options() -> Result<(), Box<dyn std::error::Error>> {
        let cmd = PuristCommand::new(Some(PathBuf::from("src")), false)
            .with_format(OutputFormat::Json)
            .with_fix(true);

        assert_that!(cmd.path(), eq(Some(Path::new("src"))));
        assert_that!(cmd.format(), eq(Some(OutputFormat::Json)));
        assert_that!(cmd.is_fix(), is_true());
        assert_that!(cmd.is_quiet(), is_false());
        Ok(())
    }
}
