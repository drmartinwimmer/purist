use clap::Parser;
use code_review_configure_lints::ConfigureLintsCommand;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "configure-lints",
    about = "Configure or remove strict Clippy lints in Cargo.toml",
    version
)]
struct Cli {
    #[command(flatten)]
    cmd: ConfigureLintsCommand,
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

#[cfg(test)]
mod tests {
    use super::*;
    use code_review_configure_lints::cargo_toml::LintProfile;
    use googletest::prelude::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(name: &str) -> Self {
            let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!("{name}_{}_{count}", std::process::id()));
            drop(fs::create_dir_all(&path));
            Self { path }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.path));
        }
    }

    #[googletest::test]
    fn run_configure_lints_with_valid_manifest_succeeds() -> Result<(), Box<dyn std::error::Error>>
    {
        let guard = TempDirGuard::new("test_configure_lints_bin");
        let manifest_path = guard.path().join("Cargo.toml");
        fs::write(
            &manifest_path,
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )?;

        let cmd = ConfigureLintsCommand::new(&manifest_path, LintProfile::Strict, false, true);
        let cli = Cli { cmd };
        let exit_code = cli.run();
        expect_that!(exit_code, eq(ExitCode::SUCCESS));

        let content = fs::read_to_string(&manifest_path)?;
        expect_that!(content, contains_substring("[lints.clippy]"));
        Ok(())
    }

    #[googletest::test]
    fn run_configure_lints_with_missing_manifest_returns_error() {
        let guard = TempDirGuard::new("test_configure_lints_missing");
        let manifest_path = guard.path().join("NonExistent.toml");

        let cmd = ConfigureLintsCommand::new(&manifest_path, LintProfile::Strict, false, true);
        let cli = Cli { cmd };
        let exit_code = cli.run();
        expect_that!(exit_code, eq(ExitCode::from(2)));
    }
}
