use clap::Parser;
use code_review_opinionated::{OpinionatedCommand, OpinionatedError};
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
        match self.cmd.run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(OpinionatedError::LintViolationsFound { .. }) => ExitCode::from(1),
            Err(err) => {
                eprintln!("Error: {err}");
                ExitCode::from(2)
            }
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    cli.run()
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::fs;
    use std::path::PathBuf;

    #[googletest::test]
    fn run_cli_clean_file_returns_success() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_cli_clean_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let file_path = temp_dir.join("clean.rs");
        fs::write(&file_path, "pub fn add(x: i32) -> i32 { x + 1 }\n")?;

        let cli = Cli {
            cmd: OpinionatedCommand::new(Some(file_path), true),
        };
        let code = cli.run();

        let _result = fs::remove_dir_all(&temp_dir);
        assert_that!(code, eq(ExitCode::SUCCESS));
        Ok(())
    }

    #[googletest::test]
    fn run_cli_with_violations_returns_exit_code_1() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_cli_viol_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let file_path = temp_dir.join("main.rs");
        fs::write(&file_path, "mod helpers { pub fn foo() {} }\n")?;

        let cli = Cli {
            cmd: OpinionatedCommand::new(Some(file_path), true),
        };
        let code = cli.run();

        let _result = fs::remove_dir_all(&temp_dir);
        assert_that!(code, eq(ExitCode::from(1)));
        Ok(())
    }

    #[googletest::test]
    fn run_cli_with_nonexistent_path_returns_exit_code_2() -> Result<(), Box<dyn std::error::Error>>
    {
        let cli = Cli {
            cmd: OpinionatedCommand::new(Some(PathBuf::from("nonexistent_path_404.rs")), true),
        };
        let code = cli.run();
        assert_that!(code, eq(ExitCode::from(2)));
        Ok(())
    }
}
