use clap::Parser;
use code_review_api::ApiCommand;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "api",
    about = "Introspect and detect public API drift against API.md",
    version
)]
struct Cli {
    #[command(flatten)]
    cmd: ApiCommand,
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
