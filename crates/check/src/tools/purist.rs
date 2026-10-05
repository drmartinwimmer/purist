use purist::{DiagnosticReport, PuristEngine};
use std::path::{Path, PathBuf};

/// Runner for purist AST static analysis rules.
pub struct PuristRunner {
    target_path: PathBuf,
}

/// Backwards compatibility alias for `PuristRunner`.
pub type OpinionatedRunner = PuristRunner;

impl PuristRunner {
    /// Creates a new `PuristRunner` targeting the specified directory or file.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes purist rules against the target path and returns the report.
    pub fn run(&self) -> Result<DiagnosticReport, std::io::Error> {
        let path = if self.target_path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            self.target_path.as_path()
        };

        let engine = PuristEngine::new();
        engine.check_path(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::fs;

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.0));
        }
    }

    #[googletest::test]
    fn run_purist_runner_on_clean_code_produces_no_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_purist_runner_clean_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("lib.rs");
        fs::write(&file_path, "pub fn calculate(a: i32) -> i32 { a * 2 }\n")?;

        let runner = PuristRunner::new(file_path);
        let report = runner.run()?;
        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn run_purist_runner_on_violation_produces_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_purist_runner_viol_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("lib.rs");
        fs::write(
            &file_path,
            "pub fn broken() -> Result<(), String> { Err(\"error\".into()) }\n",
        )?;

        let runner = PuristRunner::new(file_path);
        let report = runner.run()?;
        assert_that!(report.is_empty(), is_false());
        assert_that!(report.diagnostics.len(), eq(1));
        let diag = report.diagnostics.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::error_types"));
        Ok(())
    }
}
