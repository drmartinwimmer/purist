use clap::Parser;
use code_review::cli::{Cli, Commands};
use code_review::common::diagnostics::DiagnosticReport;
use code_review::common::reporter::render_report_with_options;
use code_review::tools::cargo_toml::{configure_lints, remove_lints};
use std::io::{self, IsTerminal};
use std::process::ExitCode;

pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_LINT_FAILURE: u8 = 1;
pub const EXIT_RUNTIME_ERROR: u8 = 2;

fn run(cli: Cli) -> Result<u8, Box<dyn std::error::Error>> {
    let report = DiagnosticReport::default();

    match &cli.command {
        Commands::ConfigureLints(args) => {
            let result = if args.remove {
                remove_lints(&args.manifest_path)?
            } else {
                configure_lints(&args.manifest_path, args.profile)?
            };

            if !cli.quiet {
                if args.remove {
                    if result.modified {
                        eprintln!(
                            "Removed {} Clippy lints from '{}'.",
                            result.lints_configured,
                            args.manifest_path.display()
                        );
                    } else {
                        eprintln!(
                            "No Clippy lints found in '{}'. Manifest unchanged.",
                            args.manifest_path.display()
                        );
                    }
                } else if result.modified {
                    eprintln!(
                        "Configured {} Clippy lints ({:?} profile) in '{}'.",
                        result.lints_configured,
                        args.profile,
                        args.manifest_path.display()
                    );
                } else {
                    eprintln!(
                        "Manifest '{}' already configured with {} Clippy lints ({:?} profile). Manifest unchanged.",
                        args.manifest_path.display(),
                        result.lints_configured,
                        args.profile
                    );
                }
            }
            return Ok(EXIT_SUCCESS);
        }
        Commands::Check(_) => {
            if !cli.quiet {
                eprintln!("Notice: check aggregator is scheduled for future milestones.");
            }
        }
        Commands::Opinionated(_) => {
            if !cli.quiet {
                eprintln!("Notice: opinionated linter is scheduled for Milestone 2.");
            }
        }
        Commands::Api(_) => {
            if !cli.quiet {
                eprintln!("Notice: api drift detector is scheduled for Milestone 3.");
            }
        }
        Commands::Coverage(_) => {
            if !cli.quiet {
                eprintln!("Notice: coverage runner is scheduled for Milestone 4.");
            }
        }
    }

    let use_color = io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    render_report_with_options(&report, cli.format, &mut io::stdout(), use_color)?;

    if report.has_errors() {
        Ok(EXIT_LINT_FAILURE)
    } else {
        Ok(EXIT_SUCCESS)
    }
}

fn is_broken_pipe(err: &(dyn std::error::Error + 'static)) -> bool {
    if let Some(io_err) = err.downcast_ref::<io::Error>() {
        return io_err.kind() == io::ErrorKind::BrokenPipe;
    }
    if let Some(code_review::tools::CargoTomlError::Io { source }) =
        err.downcast_ref::<code_review::tools::CargoTomlError>()
    {
        return source.kind() == io::ErrorKind::BrokenPipe;
    }
    false
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            if is_broken_pipe(&*err) {
                ExitCode::from(EXIT_SUCCESS)
            } else {
                eprintln!("Execution error: {err}");
                ExitCode::from(EXIT_RUNTIME_ERROR)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use code_review::cli::{ConfigureLintsArgs, LintProfile};
    use code_review::common::reporter::OutputFormat;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("main_test_{}_{}", std::process::id(), id));
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.path));
        }
    }

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
    fn test_run_configure_lints_with_valid_manifest_succeeds()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_guard = TempDirGuard::new()?;
        let manifest_path = temp_guard.path.join("Cargo.toml");
        fs::write(
            &manifest_path,
            "[package]\nname = \"main_test\"\nversion = \"0.1.0\"\n",
        )?;

        let cli = Cli {
            format: OutputFormat::Console,
            verbose: 0,
            quiet: true,
            command: Commands::ConfigureLints(ConfigureLintsArgs {
                manifest_path: manifest_path.clone(),
                profile: LintProfile::Strict,
                remove: false,
            }),
        };

        let exit_code = run(cli)?;
        ensure_eq!(exit_code, EXIT_SUCCESS);

        let content = fs::read_to_string(&manifest_path)?;
        ensure!(
            content.contains("[lints.clippy]"),
            "Expected [lints.clippy] in manifest"
        );

        Ok(())
    }

    #[test]
    fn test_run_configure_lints_with_missing_manifest_returns_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let cli = Cli {
            format: OutputFormat::Console,
            verbose: 0,
            quiet: true,
            command: Commands::ConfigureLints(ConfigureLintsArgs {
                manifest_path: PathBuf::from("/nonexistent/Cargo.toml"),
                profile: LintProfile::Strict,
                remove: false,
            }),
        };

        let result = run(cli);
        match result {
            Err(_) => Ok(()),
            Ok(code) => {
                Err(format!("Expected error, but run succeeded with exit code {code}").into())
            }
        }
    }
}
