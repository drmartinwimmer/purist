use code_review_diagnostics::DiagnosticReport;
use code_review_opinionated::OpinionatedEngine;
use std::path::{Path, PathBuf};

/// Runner for opinionated AST static analysis rules.
pub struct OpinionatedRunner {
    target_path: PathBuf,
}

impl OpinionatedRunner {
    /// Creates a new `OpinionatedRunner` targeting the specified directory or file.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes opinionated rules against the target path and returns the report.
    pub fn run(&self) -> Result<DiagnosticReport, std::io::Error> {
        let path = if self.target_path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            self.target_path.as_path()
        };

        let engine = OpinionatedEngine::new();
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
    fn run_opinionated_runner_on_clean_code_produces_no_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_op_runner_clean_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("lib.rs");
        fs::write(&file_path, "pub fn calculate(a: i32) -> i32 { a * 2 }\n")?;

        let runner = OpinionatedRunner::new(file_path);
        let report = runner.run()?;
        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn run_opinionated_runner_on_violation_produces_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_op_runner_viol_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let file_path = temp_dir.join("lib.rs");
        fs::write(
            &file_path,
            "pub fn broken() -> Result<(), String> { Err(\"error\".into()) }\n",
        )?;

        let runner = OpinionatedRunner::new(file_path);
        let report = runner.run()?;
        assert_that!(report.is_empty(), is_false());
        assert_that!(report.diagnostics.len(), eq(1));
        let diag = report.diagnostics.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("opinionated::error_types"));
        Ok(())
    }
}
