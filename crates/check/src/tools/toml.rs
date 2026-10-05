use super::file_utils::{find_files_with_extensions, is_tool_available};
use purist::{Diagnostic, Severity, Span};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml_edit::ImDocument;

/// Runner for TOML checks and formatters (Taplo or in-process toml_edit).
pub struct TomlRunner {
    target_path: PathBuf,
}

impl TomlRunner {
    /// Creates a new `TomlRunner` targeting the specified directory or file.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes TOML checks using `taplo` if available, or in-process syntax validation.
    pub fn run(&self) -> Result<Vec<Diagnostic>, std::io::Error> {
        let files = find_files_with_extensions(&self.target_path, &["toml"]);
        if files.is_empty() {
            return Ok(Vec::new());
        }

        if is_tool_available("taplo") {
            self.run_taplo(&files)
        } else {
            self.run_in_process_syntax_check(&files)
        }
    }

    fn run_taplo(&self, files: &[PathBuf]) -> Result<Vec<Diagnostic>, std::io::Error> {
        let mut cmd = Command::new("taplo");
        cmd.arg("fmt").arg("--check");
        for file in files {
            cmd.arg(file);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        Ok(parse_taplo_output(&combined, &self.target_path))
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

            if let Err(err) = ImDocument::parse(content) {
                let normalized = normalize_path(file, &self.target_path);
                let span = Span::new(normalized, 1, 1, 1, 1);
                diagnostics.push(
                    Diagnostic::new(
                        "toml::syntax",
                        Severity::Error,
                        format!("TOML syntax error in '{}': {err}", file.display()),
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

/// Parses the output of `taplo fmt --check` or `taplo check`.
pub fn parse_taplo_output(output: &str, base_dir: &Path) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();

        // Taplo fmt output: "error: file not formatted: /path/to/file.toml"
        if let Some(rest) = trimmed.strip_prefix("error: file not formatted: ") {
            let file_str = rest.trim();
            let path = PathBuf::from(file_str);
            let normalized = normalize_path(&path, base_dir);
            let span = Span::new(normalized, 1, 1, 1, 1);
            diagnostics.push(
                Diagnostic::new(
                    "fmt::toml",
                    Severity::Warning,
                    format!(
                        "TOML formatting in '{}' does not conform to taplo standards",
                        file_str
                    ),
                )
                .with_span(span)
                .with_suggested_fix(format!("Run 'taplo fmt {file_str}' to format")),
            );
        } else if let Some(rest) = trimmed.strip_prefix("error: ")
            && let Some((file_str, msg)) = rest.split_once(':')
            && file_str.ends_with(".toml")
        {
            let path = PathBuf::from(file_str.trim());
            let normalized = normalize_path(&path, base_dir);
            let span = Span::new(normalized, 1, 1, 1, 1);
            diagnostics.push(
                Diagnostic::new(
                    "toml::syntax",
                    Severity::Error,
                    format!("TOML syntax error in '{file_str}': {}", msg.trim()),
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
    fn parse_taplo_output_parses_unformatted_and_syntax_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        let sample = "\
error: file not formatted: Cargo.toml
error: invalid.toml: unexpected symbol at line 5
";
        let base = Path::new(".");
        let diags = parse_taplo_output(sample, base);
        expect_that!(diags.len(), eq(2));
        let d0 = diags.first().ok_or("missing diag 0")?;
        expect_that!(&d0.rule, eq("fmt::toml"));
        expect_that!(d0.severity, eq(Severity::Warning));
        let d1 = diags.get(1).ok_or("missing diag 1")?;
        expect_that!(&d1.rule, eq("toml::syntax"));
        expect_that!(d1.severity, eq(Severity::Error));
        Ok(())
    }

    #[googletest::test]
    fn in_process_toml_check_flags_syntax_error() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_toml_syntax_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        fs::write(temp_dir.join("valid.toml"), "[package]\nname = \"demo\"\n")?;
        fs::write(temp_dir.join("invalid.toml"), "[package\nname = demo\n")?;

        let runner = TomlRunner::new(&temp_dir);
        let files = vec![temp_dir.join("valid.toml"), temp_dir.join("invalid.toml")];
        let diags = runner.run_in_process_syntax_check(&files)?;

        expect_that!(diags.len(), eq(1));
        let d0 = diags.first().ok_or("missing diag 0")?;
        expect_that!(&d0.rule, eq("toml::syntax"));
        expect_that!(d0.severity, eq(Severity::Error));
        Ok(())
    }
}
