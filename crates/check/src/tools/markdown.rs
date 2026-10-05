use super::file_utils::{find_files_with_extensions, is_tool_available};
use purist::{Diagnostic, Severity, Span};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Runner for Markdown checks and formatters (Prettier / mdformat).
pub struct MarkdownRunner {
    target_path: PathBuf,
}

impl MarkdownRunner {
    /// Creates a new `MarkdownRunner` targeting the specified directory or file.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes markdown checks using the first available tool (`prettier` or `mdformat`).
    pub fn run(&self) -> Result<Vec<Diagnostic>, std::io::Error> {
        let files = find_files_with_extensions(&self.target_path, &["md"]);
        if files.is_empty() {
            return Ok(Vec::new());
        }

        if is_tool_available("prettier") {
            self.run_prettier(&files)
        } else if is_tool_available("mdformat") {
            self.run_mdformat(&files)
        } else {
            Ok(Vec::new())
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

        Ok(parse_prettier_markdown_output(&combined, &self.target_path))
    }

    fn run_mdformat(&self, files: &[PathBuf]) -> Result<Vec<Diagnostic>, std::io::Error> {
        let mut cmd = Command::new("mdformat");
        cmd.arg("--check");
        for file in files {
            cmd.arg(file);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        Ok(parse_mdformat_output(&combined, &self.target_path))
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

/// Parses the output of `prettier --check` on Markdown files.
pub fn parse_prettier_markdown_output(output: &str, base_dir: &Path) -> Vec<Diagnostic> {
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
                    "fmt::markdown",
                    Severity::Warning,
                    format!(
                        "Markdown formatting in '{}' does not conform to prettier standards",
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
                    "fmt::markdown",
                    Severity::Error,
                    format!("Syntax error in '{file_str}': {}", err_msg.trim()),
                )
                .with_span(span),
            );
        }
    }

    diagnostics
}

/// Parses the output of `mdformat --check`.
pub fn parse_mdformat_output(output: &str, base_dir: &Path) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        // Format: Error: File "/path/to/file.md" is not formatted.
        if let Some(rest) = trimmed.strip_prefix("Error: File \"")
            && let Some((file_str, _)) = rest.split_once("\" is not formatted")
        {
            let path = PathBuf::from(file_str);
            let normalized = normalize_path(&path, base_dir);
            let span = Span::new(normalized, 1, 1, 1, 1);
            diagnostics.push(
                Diagnostic::new(
                    "fmt::markdown",
                    Severity::Warning,
                    format!(
                        "Markdown formatting in '{}' does not conform to mdformat standards",
                        file_str
                    ),
                )
                .with_span(span)
                .with_suggested_fix(format!("Run 'mdformat {file_str}' to format")),
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
    fn parse_prettier_markdown_output_parses_warnings_and_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        let sample = "\
Checking formatting...
AGENTS.md[warn] AGENTS.md
[warn] /workspace/SPEC.md
[warn] Code style issues found in 2 files. Run Prettier with --write to fix.
[error] /workspace/broken.md: SyntaxError: Unexpected token
";
        let base = Path::new("/workspace");
        let diags = parse_prettier_markdown_output(sample, base);
        expect_that!(diags.len(), eq(3));

        let d0 = diags.first().ok_or("missing diag 0")?;
        expect_that!(&d0.rule, eq("fmt::markdown"));
        expect_that!(d0.severity, eq(Severity::Warning));
        let span0 = d0.span.as_ref().ok_or("missing span 0")?;
        expect_that!(span0.file, eq(Path::new("AGENTS.md")));

        let d1 = diags.get(1).ok_or("missing diag 1")?;
        expect_that!(&d1.rule, eq("fmt::markdown"));
        expect_that!(d1.severity, eq(Severity::Warning));
        let span1 = d1.span.as_ref().ok_or("missing span 1")?;
        expect_that!(span1.file, eq(Path::new("SPEC.md")));

        let d2 = diags.get(2).ok_or("missing diag 2")?;
        expect_that!(&d2.rule, eq("fmt::markdown"));
        expect_that!(d2.severity, eq(Severity::Error));
        let span2 = d2.span.as_ref().ok_or("missing span 2")?;
        expect_that!(span2.file, eq(Path::new("broken.md")));
        Ok(())
    }

    #[googletest::test]
    fn parse_mdformat_output_parses_unformatted_files() -> Result<(), Box<dyn std::error::Error>> {
        let sample = "\
Error: File \"/workspace/README.md\" is not formatted.
Error: File \"/workspace/AGENTS.md\" is not formatted.
";
        let base = Path::new("/workspace");
        let diags = parse_mdformat_output(sample, base);
        expect_that!(diags.len(), eq(2));

        let d0 = diags.first().ok_or("missing diag 0")?;
        expect_that!(&d0.rule, eq("fmt::markdown"));
        let span0 = d0.span.as_ref().ok_or("missing span 0")?;
        expect_that!(span0.file, eq(Path::new("README.md")));

        let d1 = diags.get(1).ok_or("missing diag 1")?;
        expect_that!(&d1.rule, eq("fmt::markdown"));
        let span1 = d1.span.as_ref().ok_or("missing span 1")?;
        expect_that!(span1.file, eq(Path::new("AGENTS.md")));
        Ok(())
    }
}
