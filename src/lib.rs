pub mod cargo;
pub mod checkers;
pub mod diagnostics;
pub mod discovery;
pub mod engine;
pub mod onboard;
pub mod reporter;
pub mod rule_config;
pub mod rules;
pub mod trackers;

pub use cargo::{LintConfig, OpinionatedLintsConfig, PuristLintsConfig, RuleLevel};
pub use checkers::{
    ReceiverKind, check_call_matches_path, check_expr_matches_path, check_fn_receiver,
    check_ident_has_negative_name, check_ident_has_prefix, check_ident_has_test_prefix,
    check_macro_matches, check_method_call_matches_name, check_path_matches,
    extract_fn_return_type, extract_result_error_type,
};
use clap::Args;
pub use diagnostics::{Diagnostic, DiagnosticReport, ReportSummary, Severity, Span};
pub use engine::{LintContext, OpinionatedEngine, PuristEngine, Rule};
pub use onboard::{canonical_rule_name, disable_rules_in_manifest, onboard_project};
pub use reporter::{OutputFormat, render_report, render_report_with_options};
pub use rules::default_rules;
use std::path::{Path, PathBuf};
pub use trackers::{
    ClapScopeTracker, DepthTracker, FlagScopeTracker, TestScopeTracker, TypeScopeTracker,
};

/// Error type for purist linter execution.
#[derive(Debug, thiserror::Error)]
pub enum PuristError {
    #[error("Target path '{0}' was not found")]
    PathNotFound(PathBuf),

    #[error("No Cargo.toml found for target path '{0}'")]
    ManifestNotFound(PathBuf),

    #[error("Failed to parse Cargo.toml: {0}")]
    ManifestParse(String),

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

    /// Perform purist checks and allow any triggered rules in Cargo.toml
    #[arg(long, alias = "onboard")]
    allow: bool,
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
            allow: false,
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

    /// Sets the allow flag.
    pub fn with_allow(mut self, allow: bool) -> Self {
        self.allow = allow;
        self
    }

    /// Sets the allow flag (alias for `with_allow`).
    pub fn with_onboard(self, onboard: bool) -> Self {
        self.with_allow(onboard)
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

    /// Returns whether allowing triggered rules in Cargo.toml is requested.
    pub fn is_allow(&self) -> bool {
        self.allow
    }

    /// Returns whether allowing triggered rules in Cargo.toml is requested.
    pub fn is_onboard(&self) -> bool {
        self.is_allow()
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
        let is_allow = self.allow;
        let is_quiet = self.quiet;
        let target_path = self.path.clone();

        let report = self.execute()?;

        render_report(&report, format, &mut std::io::stdout())?;

        if is_allow {
            let path_ref = target_path.as_deref().unwrap_or_else(|| Path::new("."));
            let disabled_count = onboard_project(path_ref, &report)?;
            if !is_quiet {
                if disabled_count > 0 {
                    println!(
                        "\nAllowed: disabled {disabled_count} triggered rule(s) in Cargo.toml."
                    );
                } else {
                    println!("\nAllowed: no rules needed to be disabled.");
                }
            }
            return Ok(());
        }

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

    #[googletest::test]
    fn verify_msrv_badge_consistency_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let cargo_toml_content = fs::read_to_string(manifest_dir.join("Cargo.toml"))?;
        let rust_version_line = cargo_toml_content
            .lines()
            .find(|line| line.trim().starts_with("rust-version"))
            .ok_or("rust-version not found in Cargo.toml")?;
        let rust_version = rust_version_line
            .split('=')
            .nth(1)
            .ok_or("Invalid rust-version entry")?
            .trim()
            .trim_matches('"');

        let mut parts = rust_version.split('.');
        let major = parts.next().ok_or("Missing major version component")?;
        let minor = parts.next().ok_or("Missing minor version component")?;
        let badge_target = format!("https://img.shields.io/badge/MSRV-{major}.{minor}%2B-");

        let readme_content = fs::read_to_string(manifest_dir.join("README.md"))?;
        assert_that!(readme_content.contains(&badge_target), is_true());

        Ok(())
    }

    #[googletest::test]
    fn parse_purist_command_with_allow_succeeds() {
        let cmd = PuristCommand::new(Some(PathBuf::from(".")), true).with_allow(true);
        assert_that!(cmd.is_allow(), is_true());
        assert_that!(cmd.is_onboard(), is_true());

        let cmd_alias = PuristCommand::new(Some(PathBuf::from(".")), true).with_onboard(true);
        assert_that!(cmd_alias.is_allow(), is_true());
    }

    #[googletest::test]
    fn run_purist_command_with_allow_disables_triggered_rules_succeeds()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_cmd_allow_{}", std::process::id()));
        fs::create_dir_all(temp_dir.join("src"))?;
        let _guard = TempDirGuard(temp_dir.clone());

        let cargo_toml = temp_dir.join("Cargo.toml");
        fs::write(
            &cargo_toml,
            r#"[package]
name = "test_onboard_app"
version = "0.1.0"
edition = "2024"
"#,
        )?;

        let bad_file = temp_dir.join("src/lib.rs");
        fs::write(
            &bad_file,
            "pub fn fail() -> Result<(), String> { Err(\"bad\".to_string()) }\n",
        )?;

        // Running without allow returns LintViolationsFound error
        let cmd_check = PuristCommand::new(Some(temp_dir.clone()), true);
        match cmd_check.run() {
            Err(PuristError::LintViolationsFound { count }) => {
                assert_that!(count, eq(1));
            }
            other => return Err(format!("Expected LintViolationsFound, got {other:?}").into()),
        }

        // Running with allow succeeds (Ok(())) and modifies Cargo.toml
        let cmd_allow = PuristCommand::new(Some(temp_dir.clone()), true).with_allow(true);
        cmd_allow.run()?;

        let updated_cargo = fs::read_to_string(&cargo_toml)?;
        assert_that!(updated_cargo, contains_substring("[lints.purist]"));
        assert_that!(updated_cargo, contains_substring("error_types = \"allow\""));

        // Running again without allow now passes with 0 violations!
        let cmd_check_again = PuristCommand::new(Some(temp_dir.clone()), true);
        cmd_check_again.run()?;

        Ok(())
    }

    #[googletest::test]
    fn run_purist_command_with_allow_on_clean_project_succeeds()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_cmd_clean_allow_{}", std::process::id()));
        fs::create_dir_all(temp_dir.join("src"))?;
        let _guard = TempDirGuard(temp_dir.clone());

        let cargo_toml = temp_dir.join("Cargo.toml");
        let initial_cargo = r#"[package]
name = "test_clean_app"
version = "0.1.0"
edition = "2024"
"#;
        fs::write(&cargo_toml, initial_cargo)?;

        let clean_file = temp_dir.join("src/lib.rs");
        fs::write(&clean_file, "pub fn add(a: i32, b: i32) -> i32 { a + b }\n")?;

        let cmd = PuristCommand::new(Some(temp_dir.clone()), true).with_allow(true);
        cmd.run()?;

        let updated_cargo = fs::read_to_string(&cargo_toml)?;
        assert_that!(updated_cargo, eq(initial_cargo));
        Ok(())
    }
}
