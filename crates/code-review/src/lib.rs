use clap::{Parser, Subcommand};
use code_review_api::{ApiCommand, ApiError};
use code_review_check::{CheckCommand, CheckError};
use code_review_configure_lints::{CargoTomlError, ConfigureLintsCommand};
use code_review_coverage::{CoverageCommand, CoverageError};
use purist::{OutputFormat, PuristCommand, PuristError};
use std::process::ExitCode;
use thiserror::Error;

/// Error type for the code-review CLI toolkit and subcommands.
#[derive(Debug, Error)]
pub enum CodeReviewError {
    /// Errors originating from the configure-lints subcommand.
    #[error(transparent)]
    ConfigureLints(#[from] CargoTomlError),

    /// Errors originating from the check aggregator subcommand.
    #[error(transparent)]
    Check(#[from] CheckError),

    /// Errors originating from the purist linter subcommand.
    #[error(transparent)]
    Purist(#[from] PuristError),

    /// Errors originating from the API drift detector subcommand.
    #[error(transparent)]
    Api(#[from] ApiError),

    /// Errors originating from the coverage runner subcommand.
    #[error(transparent)]
    Coverage(#[from] CoverageError),
}

/// Subcommands supported by the code-review CLI toolkit.
#[derive(Debug, Subcommand, PartialEq)]
pub enum Commands {
    /// Aggregates formatters, clippy, purist, audit, and coverage checks
    Check(CheckCommand),

    /// Configure or remove strict Clippy lints in Cargo.toml
    #[command(name = "configure-lints")]
    ConfigureLints(ConfigureLintsCommand),

    /// Run AST-based purist linter rules
    #[command(alias = "opinionated")]
    Purist(PuristCommand),

    /// Introspect and detect public API drift against API.md
    Api(ApiCommand),

    /// Run LLVM source-based coverage gates
    Coverage(CoverageCommand),
}

impl Commands {
    /// Executes the subcommand with default formatting.
    pub fn run(self) -> Result<(), CodeReviewError> {
        self.run_with_format(OutputFormat::Console)
    }

    /// Executes the subcommand with the specified report output format.
    pub fn run_with_format(self, format: OutputFormat) -> Result<(), CodeReviewError> {
        match self {
            Self::ConfigureLints(cmd) => Ok(cmd.run()?),
            Self::Check(mut cmd) => {
                if cmd.format().is_none() {
                    cmd = cmd.with_format(format);
                }
                Ok(cmd.run()?)
            }
            Self::Purist(mut cmd) => {
                if cmd.format().is_none() {
                    cmd = cmd.with_format(format);
                }
                Ok(cmd.run()?)
            }
            Self::Api(cmd) => Ok(cmd.run()?),
            Self::Coverage(cmd) => Ok(cmd.run()?),
        }
    }
}

/// Top-level CLI parser for code-review.
#[derive(Debug, Parser, PartialEq)]
#[command(
    name = "code-review",
    about = "Automated code review, static analysis, and lint configuration toolkit",
    version
)]
pub struct Cli {
    /// Output format for reports and diagnostics
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Console)]
    format: OutputFormat,

    /// Increase verbosity level (-v, -vv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Silence non-essential logging output
    #[arg(short, long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

impl Cli {
    /// Creates a new `Cli` instance.
    pub fn new(format: OutputFormat, verbose: u8, quiet: bool, command: Commands) -> Self {
        Self {
            format,
            verbose,
            quiet,
            command,
        }
    }

    /// Returns the configured output format.
    pub fn format(&self) -> OutputFormat {
        self.format
    }

    /// Returns the verbosity level.
    pub fn verbose(&self) -> u8 {
        self.verbose
    }

    /// Returns whether logging output is suppressed.
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Returns a reference to the selected subcommand.
    pub fn command(&self) -> &Commands {
        &self.command
    }

    /// Runs the selected subcommand and returns an exit code.
    pub fn run(self) -> ExitCode {
        match self.command.run_with_format(self.format) {
            Ok(()) => ExitCode::SUCCESS,
            Err(CodeReviewError::Check(CheckError::ViolationsFound { .. }))
            | Err(CodeReviewError::Purist(PuristError::LintViolationsFound { .. })) => {
                ExitCode::from(1)
            }
            Err(err) => {
                eprintln!("Error: {err}");
                ExitCode::from(2)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use code_review_configure_lints::LintProfile;
    use googletest::prelude::*;
    use std::path::{Path, PathBuf};

    #[googletest::test]
    fn parse_cli_default_global_flags_sets_console_format_and_zero_verbosity()
    -> Result<(), Box<dyn std::error::Error>> {
        let args = ["code-review", "check"];
        let cli = Cli::try_parse_from(args)?;
        expect_that!(cli.format(), eq(OutputFormat::Console));
        expect_that!(cli.verbose(), eq(0));
        expect_that!(cli.is_quiet(), is_false());
        expect_that!(
            cli.command(),
            eq(&Commands::Check(
                CheckCommand::default().with_format(OutputFormat::Console)
            ))
        );
        Ok(())
    }

    #[googletest::test]
    fn parse_cli_global_format_flag_sets_json_and_markdown()
    -> Result<(), Box<dyn std::error::Error>> {
        let args_json = ["code-review", "--format", "json", "check"];
        let cli_json = Cli::try_parse_from(args_json)?;
        expect_that!(cli_json.format(), eq(OutputFormat::Json));

        let args_md = ["code-review", "--format", "markdown", "check"];
        let cli_md = Cli::try_parse_from(args_md)?;
        expect_that!(cli_md.format(), eq(OutputFormat::Markdown));
        Ok(())
    }

    #[googletest::test]
    fn parse_cli_configure_lints_defaults_uses_strict_profile_and_default_manifest()
    -> Result<(), Box<dyn std::error::Error>> {
        let args = ["code-review", "configure-lints"];
        let cli = Cli::try_parse_from(args)?;
        match cli.command() {
            Commands::ConfigureLints(cmd) => {
                expect_that!(cmd.manifest_path(), eq(&PathBuf::from("Cargo.toml")));
                expect_that!(cmd.profile(), eq(LintProfile::Strict));
                expect_that!(cmd.is_remove(), is_false());
            }
            _ => return Err("Expected ConfigureLints subcommand".into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn parse_cli_configure_lints_custom_flags_parses_arguments()
    -> Result<(), Box<dyn std::error::Error>> {
        let args = [
            "code-review",
            "configure-lints",
            "--manifest-path",
            "crates/demo/Cargo.toml",
            "--profile",
            "standard",
            "--remove",
            "--quiet",
        ];
        let cli = Cli::try_parse_from(args)?;
        match cli.command() {
            Commands::ConfigureLints(cmd) => {
                expect_that!(
                    cmd.manifest_path(),
                    eq(&PathBuf::from("crates/demo/Cargo.toml"))
                );
                expect_that!(cmd.profile(), eq(LintProfile::Standard));
                expect_that!(cmd.is_remove(), is_true());
                expect_that!(cmd.is_quiet(), is_true());
            }
            _ => return Err("Expected ConfigureLints subcommand".into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn parse_cli_subcommands_dispatches_all_variants() -> Result<(), Box<dyn std::error::Error>> {
        let variants = [
            (["code-review", "check"].as_slice(), "check"),
            (
                ["code-review", "configure-lints"].as_slice(),
                "configure-lints",
            ),
            (["code-review", "purist"].as_slice(), "purist"),
            (["code-review", "opinionated"].as_slice(), "purist"),
            (["code-review", "api"].as_slice(), "api"),
            (["code-review", "coverage"].as_slice(), "coverage"),
        ];

        for (args, expected_name) in variants {
            let cli = Cli::try_parse_from(args)?;
            let actual_name = match cli.command() {
                Commands::Check(_) => "check",
                Commands::ConfigureLints(_) => "configure-lints",
                Commands::Purist(_) => "purist",
                Commands::Api(_) => "api",
                Commands::Coverage(_) => "coverage",
            };
            expect_that!(actual_name, eq(expected_name));
        }
        Ok(())
    }

    #[googletest::test]
    fn run_cli_check_command_returns_success() {
        let cmd = CheckCommand::new(None, true)
            .with_fmt(false)
            .with_clippy(false)
            .with_purist(false)
            .with_audit(false)
            .with_markdown(false)
            .with_toml(false)
            .with_json(false);
        let cli = Cli::new(OutputFormat::Console, 0, true, Commands::Check(cmd));
        expect_that!(cli.run(), eq(ExitCode::SUCCESS));
    }

    #[googletest::test]
    fn parse_cli_check_subcommand_parses_all_flags() -> Result<(), Box<dyn std::error::Error>> {
        let args = [
            "code-review",
            "--format",
            "json",
            "check",
            "--path",
            "crates/check",
            "--fail-on",
            "errors",
            "--changed-only",
            "--fmt=false",
            "--clippy=false",
            "--purist=false",
            "--audit=false",
            "--markdown=false",
            "--toml=false",
            "--json=false",
            "--quiet",
        ];
        let cli = Cli::try_parse_from(args)?;
        expect_that!(cli.format(), eq(OutputFormat::Json));
        match cli.command() {
            Commands::Check(cmd) => {
                expect_that!(cmd.path(), eq(Some(Path::new("crates/check"))));
                expect_that!(cmd.fail_on(), eq(code_review_check::FailOn::Errors));
                expect_that!(cmd.is_changed_only(), is_true());
                expect_that!(cmd.is_fmt_enabled(), is_false());
                expect_that!(cmd.is_clippy_enabled(), is_false());
                expect_that!(cmd.is_purist_enabled(), is_false());
                expect_that!(cmd.is_opinionated_enabled(), is_false());
                expect_that!(cmd.is_audit_enabled(), is_false());
                expect_that!(cmd.is_markdown_enabled(), is_false());
                expect_that!(cmd.is_toml_enabled(), is_false());
                expect_that!(cmd.is_json_enabled(), is_false());
                expect_that!(cmd.is_quiet(), is_true());
            }
            _ => return Err("Expected Check subcommand".into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn parse_cli_check_subcommand_supports_opinionated_alias()
    -> Result<(), Box<dyn std::error::Error>> {
        let args = ["code-review", "check", "--opinionated=false"];
        let cli = Cli::try_parse_from(args)?;
        match cli.command() {
            Commands::Check(cmd) => {
                expect_that!(cmd.is_purist_enabled(), is_false());
                expect_that!(cmd.is_opinionated_enabled(), is_false());
            }
            _ => return Err("Expected Check subcommand".into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn parse_cli_purist_subcommand_parses_flags() -> Result<(), Box<dyn std::error::Error>> {
        let args = [
            "code-review",
            "--format",
            "json",
            "purist",
            "--path",
            "crates/purist/src",
            "--fix",
            "--quiet",
        ];
        let cli = Cli::try_parse_from(args)?;
        expect_that!(cli.format(), eq(OutputFormat::Json));
        match cli.command() {
            Commands::Purist(cmd) => {
                expect_that!(
                    cmd.path(),
                    eq(Some(std::path::Path::new("crates/purist/src")))
                );
                expect_that!(cmd.is_fix(), is_true());
                expect_that!(cmd.is_quiet(), is_true());
            }
            _ => return Err("Expected Purist subcommand".into()),
        }
        Ok(())
    }

    struct TempDirGuard(std::path::PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(std::fs::remove_dir_all(&self.0));
        }
    }

    #[googletest::test]
    fn run_cli_purist_command_on_clean_target_returns_success()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_code_review_purist_clean_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("clean.rs");
        std::fs::write(&file_path, "pub fn helper() -> i32 { 10 }\n")?;

        let cmd = PuristCommand::new(Some(file_path), true);
        let cli = Cli::new(OutputFormat::Console, 0, true, Commands::Purist(cmd));
        let code = cli.run();

        expect_that!(code, eq(ExitCode::SUCCESS));
        Ok(())
    }

    #[googletest::test]
    fn run_cli_purist_command_with_violations_returns_exit_code_1()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!(
            "test_code_review_purist_viol_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("main.rs");
        std::fs::write(&file_path, "mod helpers { pub fn broken() {} }\n")?;

        let cmd = PuristCommand::new(Some(file_path), true);
        let cli = Cli::new(OutputFormat::Console, 0, true, Commands::Purist(cmd));
        let code = cli.run();

        expect_that!(code, eq(ExitCode::from(1)));
        Ok(())
    }
}
