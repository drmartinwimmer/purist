use clap::Parser;
use purist::{PuristCommand, PuristError};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "purist",
    about = "Fast purist AST linter for enforcing strict Rust code hygiene",
    version
)]
struct Cli {
    #[command(flatten)]
    cmd: PuristCommand,
}

impl Cli {
    fn run(self) -> ExitCode {
        match self.cmd.run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(PuristError::LintViolationsFound { .. }) => ExitCode::from(1),
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
    fn run_cli_clean_file_returns_success() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_cli_clean_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("clean.rs");
        fs::write(&file_path, "pub fn add(x: i32) -> i32 { x + 1 }\n")?;

        let cli = Cli {
            cmd: PuristCommand::new(Some(file_path), true),
        };
        let code = cli.run();

        assert_that!(code, eq(ExitCode::SUCCESS));
        Ok(())
    }

    #[googletest::test]
    fn run_cli_with_violations_returns_exit_code_1() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_cli_viol_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("main.rs");
        fs::write(&file_path, "mod helpers { pub fn foo() {} }\n")?;

        let cli = Cli {
            cmd: PuristCommand::new(Some(file_path), true),
        };
        let code = cli.run();

        assert_that!(code, eq(ExitCode::from(1)));
        Ok(())
    }

    #[googletest::test]
    fn run_cli_with_nonexistent_path_returns_exit_code_2() -> Result<(), Box<dyn std::error::Error>>
    {
        let cli = Cli {
            cmd: PuristCommand::new(Some(PathBuf::from("nonexistent_path_404.rs")), true),
        };
        let code = cli.run();
        assert_that!(code, eq(ExitCode::from(2)));
        Ok(())
    }

    #[googletest::test]
    fn run_cli_with_allow_disables_violations_and_returns_success()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_cli_allow_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let manifest_path = temp_dir.join("Cargo.toml");
        fs::write(
            &manifest_path,
            r#"[package]
name = "cli_allow"
version = "0.1.0"
edition = "2024"
"#,
        )?;
        let file_path = temp_dir.join("main.rs");
        fs::write(&file_path, "mod helpers { pub fn foo() {} }\n")?;

        let cli = Cli {
            cmd: PuristCommand::new(Some(temp_dir), true).with_allow(true),
        };
        let code = cli.run();
        assert_that!(code, eq(ExitCode::SUCCESS));
        Ok(())
    }
}
