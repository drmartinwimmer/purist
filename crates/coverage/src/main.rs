use clap::Parser;
use code_review_coverage::CoverageCommand;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "coverage",
    about = "Run LLVM source-based coverage gates",
    version
)]
struct Cli {
    #[command(flatten)]
    cmd: CoverageCommand,
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
