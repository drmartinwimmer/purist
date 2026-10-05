use purist::{Diagnostic, Severity, Span};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Runner for `cargo fmt --check`.
pub struct FmtRunner {
    target_path: PathBuf,
}

impl FmtRunner {
    /// Creates a new `FmtRunner` targeting the specified directory or workspace.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes `cargo fmt --check` and returns collected diagnostics.
    pub fn run(&self) -> Result<Vec<Diagnostic>, std::io::Error> {
        let manifest_path = if self.target_path.is_file() {
            self.target_path.clone()
        } else {
            self.target_path.join("Cargo.toml")
        };

        let mut cmd = Command::new("cargo");
        cmd.arg("fmt").arg("--all").arg("--check");

        if manifest_path.exists() {
            cmd.arg("--manifest-path").arg(&manifest_path);
        } else {
            cmd.current_dir(&self.target_path);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let combined = format!("{stdout}\n{stderr}");
        let mut diagnostics = parse_fmt_output(&combined, &self.target_path);

        if !output.status.success() && diagnostics.is_empty() {
            let error_msg = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else if !stdout.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                "Formatting check failed with non-zero exit code".to_string()
            };

            diagnostics.push(
                Diagnostic::new("fmt::formatting", Severity::Warning, error_msg)
                    .with_suggested_fix("Run 'cargo fmt' to format the codebase"),
            );
        }

        Ok(diagnostics)
    }
}

/// Parses the output of `cargo fmt --check` into structured diagnostics.
pub fn parse_fmt_output(output: &str, target_dir: &Path) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        // Standard rustfmt output: "Diff in /path/to/file.rs at line 10:"
        if let Some(rest) = trimmed.strip_prefix("Diff in ")
            && let Some((file_part, line_part)) = rest.split_once(" at line ")
        {
            let file_str = file_part.trim();
            let line_str = line_part.trim_end_matches(':').trim();
            let line_num: usize = line_str.parse().unwrap_or(1);

            let raw_path = PathBuf::from(file_str);
            let normalized_path = if raw_path.is_absolute() {
                if let Ok(rel) = raw_path.strip_prefix(target_dir) {
                    rel.to_path_buf()
                } else {
                    raw_path
                }
            } else {
                raw_path
            };

            let span = Span::new(normalized_path, line_num, 1, line_num, 1);
            diagnostics.push(
                Diagnostic::new(
                    "fmt::formatting",
                    Severity::Warning,
                    format!(
                        "Source code formatting in '{}' does not conform to rustfmt standards",
                        file_str
                    ),
                )
                .with_span(span)
                .with_suggested_fix(format!("Run 'cargo fmt' to format '{file_str}'")),
            );
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn parse_clean_fmt_output_returns_empty_diagnostics() -> Result<(), Box<dyn std::error::Error>>
    {
        let output = "";
        let diags = parse_fmt_output(output, Path::new("."));
        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn parse_diff_fmt_output_extracts_file_and_line_span() -> Result<(), Box<dyn std::error::Error>>
    {
        let output = r#"
Diff in /workspace/src/lib.rs at line 25:
-fn test() {
+fn test() {
Diff in /workspace/src/main.rs at line 12:
-let a=1;
+let a = 1;
"#;
        let diags = parse_fmt_output(output, Path::new("/workspace"));
        assert_that!(diags.len(), eq(2));

        let first = diags.first().ok_or("expected first diagnostic")?;
        assert_that!(&first.rule, eq("fmt::formatting"));
        assert_that!(first.severity, eq(Severity::Warning));
        assert_that!(
            &first.message,
            contains_substring("Source code formatting in '/workspace/src/lib.rs'")
        );

        let span = first.span.as_ref().ok_or("expected span")?;
        assert_that!(span.file, eq(&PathBuf::from("src/lib.rs")));
        assert_that!(span.start_line, eq(25));

        let second = diags.get(1).ok_or("expected second diagnostic")?;
        let span2 = second.span.as_ref().ok_or("expected span")?;
        assert_that!(span2.file, eq(&PathBuf::from("src/main.rs")));
        assert_that!(span2.start_line, eq(12));
        Ok(())
    }

    #[googletest::test]
    fn parse_relative_diff_fmt_output_preserves_path() -> Result<(), Box<dyn std::error::Error>> {
        let output = "Diff in src/foo.rs at line 42:\n";
        let diags = parse_fmt_output(output, Path::new("."));
        assert_that!(diags.len(), eq(1));

        let diag = diags.first().ok_or("expected diagnostic")?;
        let span = diag.span.as_ref().ok_or("expected span")?;
        assert_that!(span.file, eq(&PathBuf::from("src/foo.rs")));
        assert_that!(span.start_line, eq(42));
        Ok(())
    }
}
