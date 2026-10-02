use clap::Parser;
use code_review::Cli;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    cli.run()
}
