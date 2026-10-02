use clap::Parser;
use code_review::cli::{Cli, Commands};
use code_review::common::diagnostics::DiagnosticReport;
use code_review::common::reporter::render_report;
use std::io;
use std::process::ExitCode;

pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_LINT_FAILURE: u8 = 1;
pub const EXIT_RUNTIME_ERROR: u8 = 2;

fn run(cli: Cli) -> Result<u8, io::Error> {
    let report = DiagnosticReport::default();

    match &cli.command {
        Commands::ConfigureLints(args) => {
            if !cli.quiet {
                eprintln!(
                    "Notice: configure-lints for '{}' (profile: {:?}, remove: {}) is scheduled for M1-T2.",
                    args.manifest_path.display(),
                    args.profile,
                    args.remove
                );
            }
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

    render_report(&report, cli.format, &mut io::stdout())?;

    if report.has_errors() {
        Ok(EXIT_LINT_FAILURE)
    } else {
        Ok(EXIT_SUCCESS)
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("Execution error: {err}");
            ExitCode::from(EXIT_RUNTIME_ERROR)
        }
    }
}
