use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

use crate::common::reporter::OutputFormat;

pub use crate::tools::cargo_toml::LintProfile;

/// Command-line arguments for the configure-lints subcommand.
#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct ConfigureLintsArgs {
    /// Path to the Cargo.toml manifest to configure
    #[arg(long, default_value = "Cargo.toml")]
    pub manifest_path: PathBuf,

    /// Lint profile preset to inject (strict or standard)
    #[arg(long, value_enum, default_value_t = LintProfile::Strict)]
    pub profile: LintProfile,

    /// Remove configured lints instead of injecting them
    #[arg(long)]
    pub remove: bool,
}

/// Placeholder arguments for the check subcommand.
#[derive(Debug, Clone, Default, Args, PartialEq, Eq)]
pub struct CheckArgs {
    /// Path to target workspace or crate directory
    #[arg(long)]
    pub path: Option<PathBuf>,
}

/// Placeholder arguments for the opinionated subcommand.
#[derive(Debug, Clone, Default, Args, PartialEq, Eq)]
pub struct OpinionatedArgs {
    /// Path to source files or crate directory
    #[arg(long)]
    pub path: Option<PathBuf>,
}

/// Placeholder arguments for the api subcommand.
#[derive(Debug, Clone, Default, Args, PartialEq, Eq)]
pub struct ApiArgs {
    /// Path to Cargo.toml or workspace root
    #[arg(long)]
    pub manifest_path: Option<PathBuf>,
}

/// Placeholder arguments for the coverage subcommand.
#[derive(Debug, Clone, Default, Args, PartialEq)]
pub struct CoverageArgs {
    /// Minimum coverage threshold percentage
    #[arg(long)]
    pub threshold: Option<f64>,
}

/// Subcommands supported by the code-review CLI toolkit.
#[derive(Debug, Subcommand, PartialEq)]
pub enum Commands {
    /// Aggregates formatters, clippy, opinionated, audit, and coverage checks
    Check(CheckArgs),

    /// Configure or remove strict Clippy lints in Cargo.toml
    #[command(name = "configure-lints")]
    ConfigureLints(ConfigureLintsArgs),

    /// Run AST-based opinionated linter rules
    Opinionated(OpinionatedArgs),

    /// Introspect and detect public API drift against API.md
    Api(ApiArgs),

    /// Run LLVM source-based coverage gates
    Coverage(CoverageArgs),
}

/// Code review toolkit CLI parser.
#[derive(Debug, Parser)]
#[command(
    name = "code-review",
    about = "Automated code review, static analysis, and lint configuration toolkit",
    version
)]
pub struct Cli {
    /// Output format for reports and diagnostics
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Console)]
    pub format: OutputFormat,

    /// Increase verbosity level (-v, -vv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Silence non-essential logging output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! ensure {
        ($cond:expr, $($arg:tt)*) => {
            if !$cond {
                return Err(format!($($arg)*).into());
            }
        };
    }

    macro_rules! ensure_eq {
        ($left:expr, $right:expr) => {
            if $left != $right {
                return Err(
                    format!("check failed: left: `{:?}`, right: `{:?}`", $left, $right).into(),
                );
            }
        };
    }

    #[test]
    fn test_parse_cli_default_global_flags_sets_console_format_and_zero_verbosity()
    -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["code-review", "check"])?;
        ensure_eq!(cli.format, OutputFormat::Console);
        ensure_eq!(cli.verbose, 0);
        ensure!(!cli.quiet, "quiet should default to false");
        ensure_eq!(cli.command, Commands::Check(CheckArgs::default()));
        Ok(())
    }

    #[test]
    fn test_parse_cli_configure_lints_defaults_uses_strict_profile_and_default_manifest()
    -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from(["code-review", "configure-lints"])?;
        match cli.command {
            Commands::ConfigureLints(args) => {
                ensure_eq!(args.manifest_path, PathBuf::from("Cargo.toml"));
                ensure_eq!(args.profile, LintProfile::Strict);
                ensure!(!args.remove, "remove flag should default to false");
            }
            _ => return Err("Expected ConfigureLints subcommand".into()),
        }
        Ok(())
    }

    #[test]
    fn test_parse_cli_configure_lints_custom_flags_parses_arguments()
    -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli::try_parse_from([
            "code-review",
            "configure-lints",
            "--manifest-path",
            "crates/sub/Cargo.toml",
            "--profile",
            "standard",
            "--remove",
        ])?;
        match cli.command {
            Commands::ConfigureLints(args) => {
                ensure_eq!(args.manifest_path, PathBuf::from("crates/sub/Cargo.toml"));
                ensure_eq!(args.profile, LintProfile::Standard);
                ensure!(args.remove, "remove flag should be true");
            }
            _ => return Err("Expected ConfigureLints subcommand".into()),
        }
        Ok(())
    }

    #[test]
    fn test_parse_cli_subcommands_dispatches_all_variants() -> Result<(), Box<dyn std::error::Error>>
    {
        let check_cli = Cli::try_parse_from(["code-review", "check", "--path", "crates/foo"])?;
        ensure_eq!(
            check_cli.command,
            Commands::Check(CheckArgs {
                path: Some(PathBuf::from("crates/foo"))
            })
        );

        let opinionated_cli = Cli::try_parse_from(["code-review", "opinionated"])?;
        ensure_eq!(
            opinionated_cli.command,
            Commands::Opinionated(OpinionatedArgs::default())
        );

        let api_cli = Cli::try_parse_from(["code-review", "api"])?;
        ensure_eq!(api_cli.command, Commands::Api(ApiArgs::default()));

        let coverage_cli = Cli::try_parse_from(["code-review", "coverage"])?;
        ensure_eq!(
            coverage_cli.command,
            Commands::Coverage(CoverageArgs::default())
        );
        Ok(())
    }

    #[test]
    fn test_parse_cli_global_format_flag_sets_json_and_markdown()
    -> Result<(), Box<dyn std::error::Error>> {
        let json_cli = Cli::try_parse_from(["code-review", "--format", "json", "check"])?;
        ensure_eq!(json_cli.format, OutputFormat::Json);

        let md_cli = Cli::try_parse_from(["code-review", "--format", "markdown", "check"])?;
        ensure_eq!(md_cli.format, OutputFormat::Markdown);

        let verbose_cli = Cli::try_parse_from(["code-review", "-vv", "-q", "check"])?;
        ensure_eq!(verbose_cli.verbose, 2);
        ensure!(verbose_cli.quiet, "quiet flag should be true");
        Ok(())
    }
}
