use clap::Parser;
use code_review_opinionated::OpinionatedCommand;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "opinionated",
    about = "Run AST-based opinionated linter rules",
    version
)]
struct Cli {
    #[command(flatten)]
    cmd: OpinionatedCommand,
}

impl Cli {
    fn run(&self) -> ExitCode {
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
