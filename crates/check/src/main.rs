use clap::Parser;
use code_review_check::CheckCommand;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "check",
    about = "Aggregates formatters, clippy, opinionated, audit, and coverage checks",
    version
)]
struct Cli {
    #[command(flatten)]
    cmd: CheckCommand,
}

impl Cli {
    fn run(self) -> ExitCode {
        if let Err(err) = self.cmd.run() {
            eprintln!("Error: {err}");
            ExitCode::from(2)
        } else {
            ExitCode::SUCCESS
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    cli.run()
}
