use super::file_utils::{find_files_with_extensions, is_tool_available};
use purist::{Diagnostic, Severity, Span};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Runner for JSON checks and formatters (Prettier or in-process serde_json).
pub struct JsonRunner {
    target_path: PathBuf,
}

impl JsonRunner {
    /// Creates a new `JsonRunner` targeting the specified directory or file.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes JSON checks using `prettier` if available, or in-process syntax validation.
    pub fn run(&self) -> Result<Vec<Diagnostic>, std::io::Error> {
        let files = find_files_with_extensions(&self.target_path, &["json"]);
        if files.is_empty() {
            return Ok(Vec::new());
        }

        if is_tool_available("prettier") {
            self.run_prettier(&files)
        } else {
            self.run_in_process_syntax_check(&files)
        }
    }

    fn run_prettier(&self, files: &[PathBuf]) -> Result<Vec<Diagnostic>, std::io::Error> {
        let mut cmd = Command::new("prettier");
        cmd.arg("--check");
        for file in files {
            cmd.arg(file);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        Ok(parse_prettier_json_output(&combined, &self.target_path))
    }

    fn run_in_process_syntax_check(
        &self,
        files: &[PathBuf],
    ) -> Result<Vec<Diagnostic>, std::io::Error> {
        let mut diagnostics = Vec::new();

        for file in files {
            let Ok(content) = fs::read_to_string(file) else {
                continue;
            };

            if let Err(err) = serde_json::from_str::<serde_json::Value>(&content) {
                let line = err.line();
                let col = err.column();
                let normalized = normalize_path(file, &self.target_path);
                let span = Span::new(normalized, line, col, line, col);
                diagnostics.push(
                    Diagnostic::new(
                        "json::syntax",
                        Severity::Error,
                        format!("JSON syntax error in '{}': {err}", file.display()),
                    )
                    .with_span(span),
                );
            }
        }

        Ok(diagnostics)
    }
}

/// Normalizes a file path relative to the base target directory.
fn normalize_path(path: &Path, base_dir: &Path) -> PathBuf {
    if path.is_absolute() {
        if let Ok(rel) = path.strip_prefix(base_dir) {
            rel.to_path_buf()
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    }
}

/// Parses the output of `prettier --check` on JSON files.
pub fn parse_prettier_json_output(output: &str, base_dir: &Path) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();

        if let Some((_, rest)) = trimmed.split_once("[warn] ") {
            if rest.starts_with("Code style issues found") {
                continue;
            }
            let file_str = rest.trim();
            if file_str.is_empty() {
                continue;
            }

            let path = PathBuf::from(file_str);
            let normalized = normalize_path(&path, base_dir);
            let span = Span::new(normalized, 1, 1, 1, 1);
            diagnostics.push(
                Diagnostic::new(
                    "fmt::json",
                    Severity::Warning,
                    format!(
                        "JSON formatting in '{}' does not conform to prettier standards",
                        file_str
                    ),
                )
                .with_span(span)
                .with_suggested_fix(format!("Run 'prettier --write {file_str}' to format")),
            );
        } else if let Some((_, rest)) = trimmed.split_once("[error] ")
            && let Some((file_part, err_msg)) = rest.split_once(':')
        {
            let file_str = file_part.trim();
            let path = PathBuf::from(file_str);
            let normalized = normalize_path(&path, base_dir);
            let span = Span::new(normalized, 1, 1, 1, 1);
            diagnostics.push(
                Diagnostic::new(
                    "json::syntax",
                    Severity::Error,
                    format!("Syntax error in '{file_str}': {}", err_msg.trim()),
                )
                .with_span(span),
            );
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.0));
        }
    }

    #[googletest::test]
    fn parse_prettier_json_output_parses_warnings_and_syntax_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        let sample = "\
Checking formatting...
[warn] package.json
[warn] Code style issues found in 1 file. Run Prettier with --write to fix.
[error] bad.json: SyntaxError: Unexpected token
";
        let base = Path::new(".");
        let diags = parse_prettier_json_output(sample, base);
        expect_that!(diags.len(), eq(2));
        let d0 = diags.first().ok_or("missing diag 0")?;
        expect_that!(&d0.rule, eq("fmt::json"));
        expect_that!(d0.severity, eq(Severity::Warning));
        let d1 = diags.get(1).ok_or("missing diag 1")?;
        expect_that!(&d1.rule, eq("json::syntax"));
        expect_that!(d1.severity, eq(Severity::Error));
        Ok(())
    }

    #[googletest::test]
    fn in_process_syntax_check_flags_syntax_error() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_json_syntax_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        fs::write(temp_dir.join("valid.json"), "{\"name\": \"test\"}")?;
        fs::write(temp_dir.join("invalid.json"), "{ invalid: json }")?;

        let runner = JsonRunner::new(&temp_dir);
        let files = vec![temp_dir.join("valid.json"), temp_dir.join("invalid.json")];
        let diags = runner.run_in_process_syntax_check(&files)?;

        expect_that!(diags.len(), eq(1));
        let d0 = diags.first().ok_or("missing diag 0")?;
        expect_that!(&d0.rule, eq("json::syntax"));
        expect_that!(d0.severity, eq(Severity::Error));
        Ok(())
    }
}
