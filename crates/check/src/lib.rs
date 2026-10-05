pub mod tools;

use clap::Args;
use purist::{DiagnosticReport, OutputFormat, render_report};
use std::path::{Path, PathBuf};
pub use tools::{
    AuditRunner, ClippyRunner, FmtRunner, JjError, JjVcs, JsonRunner, MarkdownRunner,
    OpinionatedRunner, PuristRunner, TomlRunner, aggregate_diagnostics,
    filter_diagnostics_by_changed_files,
};

/// Severity threshold triggering non-zero exit code.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    clap::ValueEnum,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum FailOn {
    /// Fail if any warnings or errors are found.
    #[default]
    Warnings,
    /// Fail only if errors are found.
    Errors,
}

/// Error type for check aggregator execution.
#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("Target path '{0}' was not found")]
    PathNotFound(PathBuf),

    #[error("Jujutsu VCS error: {0}")]
    Vcs(#[from] JjError),

    #[error("I/O error during check execution: {0}")]
    Io(#[from] std::io::Error),

    #[error("Check violations found ({count} issues exceed failure threshold)")]
    ViolationsFound { count: usize },
}

/// Arguments for the check aggregator subcommand.
#[derive(Args, Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckCommand {
    /// Path to target workspace or crate directory
    #[arg(long)]
    path: Option<PathBuf>,

    /// Output format for reports and diagnostics
    #[arg(long, value_enum)]
    format: Option<OutputFormat>,

    /// Severity threshold triggering non-zero exit code
    #[arg(long, value_enum, default_value_t = FailOn::Warnings)]
    fail_on: FailOn,

    /// Filter diagnostics to only files modified in Jujutsu working copy
    #[arg(long)]
    changed_only: bool,

    /// Skip running cargo fmt
    #[arg(long)]
    skip_fmt: bool,

    /// Skip running cargo clippy
    #[arg(long)]
    skip_clippy: bool,

    /// Skip running purist AST linter
    #[arg(long, alias = "skip-opinionated")]
    skip_purist: bool,

    /// Skip running cargo audit
    #[arg(long)]
    skip_audit: bool,

    /// Skip running markdown format/lint checks
    #[arg(long)]
    skip_markdown: bool,

    /// Skip running TOML format/lint checks
    #[arg(long)]
    skip_toml: bool,

    /// Skip running JSON format/lint checks
    #[arg(long)]
    skip_json: bool,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl CheckCommand {
    /// Creates a new `CheckCommand` instance with default options.
    pub fn new(path: Option<PathBuf>, quiet: bool) -> Self {
        Self {
            path,
            format: None,
            fail_on: FailOn::Warnings,
            changed_only: false,
            skip_fmt: false,
            skip_clippy: false,
            skip_purist: false,
            skip_audit: false,
            skip_markdown: false,
            skip_toml: false,
            skip_json: false,
            quiet,
        }
    }

    /// Sets the output format.
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = Some(format);
        self
    }

    /// Sets the failure threshold.
    pub fn with_fail_on(mut self, fail_on: FailOn) -> Self {
        self.fail_on = fail_on;
        self
    }

    /// Sets changed-only filtering.
    pub fn with_changed_only(mut self, changed_only: bool) -> Self {
        self.changed_only = changed_only;
        self
    }

    /// Enables or disables cargo fmt checks.
    pub fn with_fmt(mut self, enabled: bool) -> Self {
        self.skip_fmt = !enabled;
        self
    }

    /// Enables or disables cargo clippy checks.
    pub fn with_clippy(mut self, enabled: bool) -> Self {
        self.skip_clippy = !enabled;
        self
    }

    /// Enables or disables purist AST linter checks.
    pub fn with_purist(mut self, enabled: bool) -> Self {
        self.skip_purist = !enabled;
        self
    }

    /// Backwards compatibility alias for `with_purist`.
    pub fn with_opinionated(self, enabled: bool) -> Self {
        self.with_purist(enabled)
    }

    /// Enables or disables cargo audit dependency security scan.
    pub fn with_audit(mut self, enabled: bool) -> Self {
        self.skip_audit = !enabled;
        self
    }

    /// Enables or disables markdown format/lint checks.
    pub fn with_markdown(mut self, enabled: bool) -> Self {
        self.skip_markdown = !enabled;
        self
    }

    /// Enables or disables TOML format/lint checks.
    pub fn with_toml(mut self, enabled: bool) -> Self {
        self.skip_toml = !enabled;
        self
    }

    /// Enables or disables JSON format/lint checks.
    pub fn with_json(mut self, enabled: bool) -> Self {
        self.skip_json = !enabled;
        self
    }

    /// Returns the target path, if specified.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the configured output format, if specified.
    pub fn format(&self) -> Option<OutputFormat> {
        self.format
    }

    /// Returns the failure threshold.
    pub fn fail_on(&self) -> FailOn {
        self.fail_on
    }

    /// Returns whether changed-only filtering is requested.
    pub fn is_changed_only(&self) -> bool {
        self.changed_only
    }

    /// Returns whether cargo fmt is enabled.
    pub fn is_fmt_enabled(&self) -> bool {
        !self.skip_fmt
    }

    /// Returns whether cargo clippy is enabled.
    pub fn is_clippy_enabled(&self) -> bool {
        !self.skip_clippy
    }

    /// Returns whether purist AST linter is enabled.
    pub fn is_purist_enabled(&self) -> bool {
        !self.skip_purist
    }

    /// Backwards compatibility alias for `is_purist_enabled`.
    pub fn is_opinionated_enabled(&self) -> bool {
        !self.skip_purist
    }

    /// Returns whether cargo audit is enabled.
    pub fn is_audit_enabled(&self) -> bool {
        !self.skip_audit
    }

    /// Returns whether markdown checks are enabled.
    pub fn is_markdown_enabled(&self) -> bool {
        !self.skip_markdown
    }

    /// Returns whether TOML checks are enabled.
    pub fn is_toml_enabled(&self) -> bool {
        !self.skip_toml
    }

    /// Returns whether JSON checks are enabled.
    pub fn is_json_enabled(&self) -> bool {
        !self.skip_json
    }

    /// Returns whether logging output is suppressed.
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Executes all configured checking tools and returns the aggregated diagnostic report.
    pub fn execute(self) -> Result<DiagnosticReport, CheckError> {
        let target_dir = self.path.as_deref().unwrap_or_else(|| Path::new("."));

        if !target_dir.exists() {
            return Err(CheckError::PathNotFound(target_dir.to_path_buf()));
        }

        let fmt_diags = if self.skip_fmt {
            Vec::new()
        } else {
            let runner = FmtRunner::new(target_dir);
            runner.run()?
        };

        let clippy_diags = if self.skip_clippy {
            Vec::new()
        } else {
            let runner = ClippyRunner::new(target_dir);
            runner.run()?
        };

        let purist_report = if self.skip_purist {
            DiagnosticReport::default()
        } else {
            let runner = PuristRunner::new(target_dir);
            runner.run()?
        };

        let audit_diags = if self.skip_audit {
            Vec::new()
        } else {
            let runner = AuditRunner::new(target_dir);
            runner.run()?
        };

        let markdown_diags = if self.skip_markdown {
            Vec::new()
        } else {
            let runner = MarkdownRunner::new(target_dir);
            runner.run()?
        };

        let toml_diags = if self.skip_toml {
            Vec::new()
        } else {
            let runner = TomlRunner::new(target_dir);
            runner.run()?
        };

        let json_diags = if self.skip_json {
            Vec::new()
        } else {
            let runner = JsonRunner::new(target_dir);
            runner.run()?
        };

        let aggregated = aggregate_diagnostics(
            fmt_diags,
            clippy_diags,
            purist_report,
            audit_diags,
            markdown_diags,
            toml_diags,
            json_diags,
        );

        if self.changed_only {
            let vcs = JjVcs::new(target_dir);
            let changed_files = vcs.query_changed_files()?;
            Ok(filter_diagnostics_by_changed_files(
                aggregated,
                &changed_files,
                target_dir,
            ))
        } else {
            Ok(aggregated)
        }
    }

    /// Runs the check aggregator, rendering reports and returning violation errors.
    pub fn run(self) -> Result<(), CheckError> {
        let format = self.format.unwrap_or(OutputFormat::Console);
        let fail_on = self.fail_on;
        let report = self.execute()?;

        render_report(&report, format, &mut std::io::stdout())?;

        let fails = match fail_on {
            FailOn::Warnings => report.has_errors() || report.warning_count() > 0,
            FailOn::Errors => report.has_errors(),
        };

        if fails {
            let count = match fail_on {
                FailOn::Warnings => report.error_count() + report.warning_count(),
                FailOn::Errors => report.error_count(),
            };
            Err(CheckError::ViolationsFound { count })
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
    fn parse_check_command_with_flags_populates_fields() -> Result<(), Box<dyn std::error::Error>> {
        let cmd = CheckCommand::new(Some(PathBuf::from("crates/check")), false)
            .with_format(OutputFormat::Json)
            .with_fail_on(FailOn::Errors)
            .with_changed_only(true)
            .with_fmt(false)
            .with_clippy(false)
            .with_purist(false)
            .with_audit(false)
            .with_markdown(false)
            .with_toml(false)
            .with_json(false);

        assert_that!(cmd.path(), eq(Some(Path::new("crates/check"))));
        assert_that!(cmd.format(), eq(Some(OutputFormat::Json)));
        assert_that!(cmd.fail_on(), eq(FailOn::Errors));
        assert_that!(cmd.is_changed_only(), is_true());
        assert_that!(cmd.is_fmt_enabled(), is_false());
        assert_that!(cmd.is_clippy_enabled(), is_false());
        assert_that!(cmd.is_purist_enabled(), is_false());
        assert_that!(cmd.is_opinionated_enabled(), is_false());
        assert_that!(cmd.is_audit_enabled(), is_false());
        assert_that!(cmd.is_markdown_enabled(), is_false());
        assert_that!(cmd.is_toml_enabled(), is_false());
        assert_that!(cmd.is_json_enabled(), is_false());
        assert_that!(cmd.is_quiet(), is_false());
        Ok(())
    }

    #[googletest::test]
    fn execute_on_missing_path_returns_path_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let missing = PathBuf::from("non_existent_dir_99999");
        let cmd = CheckCommand::new(Some(missing.clone()), true);

        match cmd.execute() {
            Err(CheckError::PathNotFound(p)) => {
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
    fn execute_with_all_checks_skipped_returns_empty_report()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_check_all_skipped_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        let cmd = CheckCommand::new(Some(temp_dir), true)
            .with_fmt(false)
            .with_clippy(false)
            .with_purist(false)
            .with_audit(false)
            .with_markdown(false)
            .with_toml(false)
            .with_json(false);

        let report = cmd.execute()?;
        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn run_with_skipped_checks_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_check_run_ok_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        let cmd = CheckCommand::new(Some(temp_dir), true)
            .with_fmt(false)
            .with_clippy(false)
            .with_purist(false)
            .with_audit(false)
            .with_markdown(false)
            .with_toml(false)
            .with_json(false);

        assert_that!(cmd.run(), ok(anything()));
        Ok(())
    }

    #[googletest::test]
    fn run_with_violation_and_fail_on_warnings_fails() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_check_viol_warn_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let bad_file = temp_dir.join("lib.rs");
        fs::write(
            &bad_file,
            "pub fn fail() -> Result<(), String> { Err(\"bad\".to_string()) }\n",
        )?;

        let cmd = CheckCommand::new(Some(temp_dir), true)
            .with_fmt(false)
            .with_clippy(false)
            .with_audit(false)
            .with_markdown(false)
            .with_toml(false)
            .with_json(false)
            .with_fail_on(FailOn::Warnings);

        match cmd.run() {
            Err(CheckError::ViolationsFound { count }) => {
                assert_that!(count, eq(1));
            }
            other => return Err(format!("Expected ViolationsFound, got {other:?}").into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn run_with_warning_and_fail_on_errors_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_check_viol_err_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let bad_file = temp_dir.join("lib.rs");
        fs::write(
            &bad_file,
            "pub fn fail() -> Result<(), String> { Err(\"bad\".to_string()) }\n",
        )?;

        let cmd = CheckCommand::new(Some(temp_dir), true)
            .with_fmt(false)
            .with_clippy(false)
            .with_audit(false)
            .with_markdown(false)
            .with_toml(false)
            .with_json(false)
            .with_fail_on(FailOn::Errors);

        assert_that!(cmd.run(), ok(anything()));
        Ok(())
    }
}
