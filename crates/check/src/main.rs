use clap::Parser;
use code_review_check::{CheckCommand, CheckError};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "check",
    about = "Aggregates formatters, clippy, purist, audit, and coverage checks",
    version
)]
struct Cli {
    #[command(flatten)]
    cmd: CheckCommand,
}

impl Cli {
    fn run(self) -> ExitCode {
        match self.cmd.run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(CheckError::ViolationsFound { .. }) => ExitCode::from(1),
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

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.0));
        }
    }

    #[googletest::test]
    fn run_cli_clean_target_returns_success() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_check_cli_clean_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        let cmd = CheckCommand::new(Some(temp_dir), true)
            .with_fmt(false)
            .with_clippy(false)
            .with_purist(false)
            .with_audit(false)
            .with_markdown(false)
            .with_toml(false)
            .with_json(false);

        let cli = Cli { cmd };
        assert_that!(cli.run(), eq(ExitCode::SUCCESS));
        Ok(())
    }

    #[googletest::test]
    fn run_cli_missing_path_returns_exit_code_2() {
        let cmd = CheckCommand::new(Some(PathBuf::from("nonexistent_path_8888")), true);
        let cli = Cli { cmd };
        assert_that!(cli.run(), eq(ExitCode::from(2)));
    }
}
