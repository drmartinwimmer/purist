use code_review_diagnostics::{Diagnostic, DiagnosticReport, Severity, Span};
use proc_macro2::Span as ProcMacroSpan;
use std::fs;
use std::path::{Path, PathBuf};

/// Evaluation context provided to each rule during file analysis.
#[derive(Debug)]
pub struct LintContext<'a> {
    file_path: &'a Path,
    source: &'a str,
    lines: Vec<&'a str>,
    is_main_or_lib: bool,
    is_test_file: bool,
}

impl<'a> LintContext<'a> {
    /// Creates a new `LintContext` for a source file.
    pub fn new(file_path: &'a Path, source: &'a str) -> Self {
        let lines: Vec<&'a str> = source.lines().collect();
        let file_name = file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();

        let is_main_or_lib = file_name == "main.rs"
            || file_name == "lib.rs"
            || file_path.components().any(|c| c.as_os_str() == "bin");

        let is_test_file = file_path
            .components()
            .any(|c| c.as_os_str() == "tests" || c.as_os_str() == "test")
            || file_name.ends_with("_test.rs")
            || file_name.starts_with("test_");

        Self {
            file_path,
            source,
            lines,
            is_main_or_lib,
            is_test_file,
        }
    }

    /// Returns the target file path.
    pub fn file_path(&self) -> &Path {
        self.file_path
    }

    /// Returns the raw source code.
    pub fn source(&self) -> &str {
        self.source
    }

    /// Returns the split source lines.
    pub fn lines(&self) -> &[&'a str] {
        &self.lines
    }

    /// Returns whether this file is an entry point (`main.rs` or `lib.rs`).
    pub fn is_main_or_lib(&self) -> bool {
        self.is_main_or_lib
    }

    /// Returns whether this file is an integration test or test file.
    pub fn is_test_file(&self) -> bool {
        self.is_test_file
    }

    /// Returns the 1-indexed line content, if within bounds.
    pub fn line_content(&self, line_number: usize) -> Option<&'a str> {
        if line_number == 0 || line_number > self.lines.len() {
            None
        } else {
            self.lines.get(line_number - 1).copied()
        }
    }

    /// Converts a `proc_macro2::Span` into a `code_review_diagnostics::Span`.
    pub fn to_span(&self, span: ProcMacroSpan) -> Span {
        let start = span.start();
        let end = span.end();
        Span::new(
            self.file_path,
            start.line,
            start.column + 1,
            end.line,
            end.column + 1,
        )
    }

    /// Checks whether the line immediately preceding `line_number` contains an explanatory comment.
    /// Blank lines immediately preceding the target are ignored when searching upward for a comment.
    pub fn has_preceding_comment(&self, line_number: usize) -> bool {
        if line_number <= 1 {
            return false;
        }

        let mut current_line = line_number - 1;
        while current_line >= 1 {
            if let Some(line) = self.line_content(current_line) {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    current_line -= 1;
                    continue;
                }
                return trimmed.starts_with("//")
                    || trimmed.starts_with("/*")
                    || trimmed.starts_with('*');
            }
            break;
        }
        false
    }
}

/// A static analysis rule capable of inspecting parsed Rust syntax.
pub trait Rule: Send + Sync {
    /// Returns the unique rule identifier (e.g. `opinionated::no_inline_mods`).
    fn name(&self) -> &'static str;

    /// Analyzes the parsed AST file and reports any diagnostics found.
    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic>;
}

/// Static analysis engine running registered opinionated rules over Rust source files.
pub struct OpinionatedEngine {
    rules: Vec<Box<dyn Rule>>,
}

impl Default for OpinionatedEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl OpinionatedEngine {
    /// Creates a new engine instance with no rules registered.
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Creates a new engine instance with default opinionated rules registered.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Registers a custom or built-in rule.
    pub fn with_rule(mut self, rule: Box<dyn Rule>) -> Self {
        self.rules.push(rule);
        self
    }

    /// Registers multiple rules.
    pub fn with_rules(mut self, rules: Vec<Box<dyn Rule>>) -> Self {
        self.rules.extend(rules);
        self
    }

    /// Returns a slice of all registered rules.
    pub fn rules(&self) -> &[Box<dyn Rule>] {
        &self.rules
    }

    /// Analyzes a single in-memory source string without disk access.
    pub fn check_source(&self, file_path: &Path, source: &str) -> DiagnosticReport {
        let mut report = DiagnosticReport::default();
        let ctx = LintContext::new(file_path, source);

        match syn::parse_file(source) {
            Ok(ast) => {
                for rule in &self.rules {
                    let diags = rule.check_file(&ctx, &ast);
                    for diag in diags {
                        report.add(diag);
                    }
                }
            }
            Err(parse_err) => {
                let span = ctx.to_span(parse_err.span());
                report.add(
                    Diagnostic::new(
                        "opinionated::syntax_error",
                        Severity::Error,
                        format!("Failed to parse Rust syntax: {parse_err}"),
                    )
                    .with_span(span),
                );
            }
        }

        report
    }

    /// Analyzes all Rust files in the specified path (file or directory).
    pub fn check_path(&self, target_path: &Path) -> Result<DiagnosticReport, std::io::Error> {
        let mut report = DiagnosticReport::default();
        let mut files = Vec::new();

        collect_rust_files(target_path, &mut files)?;
        files.sort();

        let mut targets_scanned = 0;
        for path in &files {
            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(err) => {
                    report.add(
                        Diagnostic::new(
                            "opinionated::io_error",
                            Severity::Error,
                            format!("Failed to read file '{}': {err}", path.display()),
                        )
                        .with_span(Span::new(path, 1, 1, 1, 1)),
                    );
                    continue;
                }
            };

            targets_scanned += 1;
            let file_report = self.check_source(path, &content);
            for diag in file_report.diagnostics {
                report.add(diag);
            }
        }

        report.summary.targets_scanned = targets_scanned;
        Ok(report)
    }
}

/// Recursively collects `.rs` files while pruning build artifacts and VCS metadata.
fn collect_rust_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Path '{}' does not exist", path.display()),
        ));
    }

    if path.is_file() {
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }

    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();

    // Skip build directories, VCS directories, and hidden directories
    if file_name == "target"
        || file_name == ".git"
        || file_name == ".jj"
        || file_name == ".direnv"
        || file_name == ".cargo"
        || (file_name.starts_with('.') && !file_name.is_empty())
    {
        return Ok(());
    }

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            let dir_name = entry.file_name();
            let dir_name_str = dir_name.to_str().unwrap_or_default();
            if dir_name_str == "target"
                || dir_name_str == ".git"
                || dir_name_str == ".jj"
                || dir_name_str == ".direnv"
                || dir_name_str == ".cargo"
                || dir_name_str.starts_with('.')
            {
                continue;
            }
            collect_rust_files(&entry_path, files)?;
        } else if entry_path.extension().and_then(|s| s.to_str()) == Some("rs") {
            files.push(entry_path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    struct DummyRule;
    impl Rule for DummyRule {
        fn name(&self) -> &'static str {
            "opinionated::dummy"
        }

        fn check_file(&self, ctx: &LintContext<'_>, _file: &syn::File) -> Vec<Diagnostic> {
            vec![
                Diagnostic::new(self.name(), Severity::Warning, "Dummy finding for testing")
                    .with_span(Span::new(ctx.file_path(), 1, 1, 1, 5)),
            ]
        }
    }

    #[googletest::test]
    fn test_lint_context_detects_entry_points_and_comments() -> googletest::Result<()> {
        let source = "// Preceding explanation\nfn main() {}\n";
        let ctx = LintContext::new(Path::new("src/main.rs"), source);

        assert_that!(ctx.is_main_or_lib(), is_true());
        assert_that!(ctx.is_test_file(), is_false());
        assert_that!(ctx.has_preceding_comment(2), is_true());
        assert_that!(ctx.has_preceding_comment(1), is_false());
        assert_that!(ctx.line_content(1), eq(Some("// Preceding explanation")));
        assert_that!(ctx.line_content(2), eq(Some("fn main() {}")));
        assert_that!(ctx.line_content(3), eq(None));
        Ok(())
    }

    #[googletest::test]
    fn test_lint_context_detects_test_file() -> googletest::Result<()> {
        let source = "fn helper() {}\n";
        let ctx = LintContext::new(Path::new("tests/integration_test.rs"), source);

        assert_that!(ctx.is_main_or_lib(), is_false());
        assert_that!(ctx.is_test_file(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn test_engine_check_source_syntax_error_produces_diagnostic() -> googletest::Result<()> {
        let engine = OpinionatedEngine::empty();
        let report = engine.check_source(Path::new("bad.rs"), "fn broken syntax {{{");

        assert_that!(report.has_errors(), is_true());
        assert_that!(report.diagnostics.len(), eq(1));
        assert_that!(
            report.diagnostics[0].rule.as_str(),
            eq("opinionated::syntax_error")
        );
        Ok(())
    }

    #[googletest::test]
    fn test_engine_runs_registered_rule() -> googletest::Result<()> {
        let engine = OpinionatedEngine::empty().with_rule(Box::new(DummyRule));
        let report = engine.check_source(Path::new("clean.rs"), "fn ok() {}\n");

        assert_that!(report.has_errors(), is_false());
        assert_that!(report.warning_count(), eq(1));
        assert_that!(
            report.diagnostics[0].rule.as_str(),
            eq("opinionated::dummy")
        );
        Ok(())
    }

    #[googletest::test]
    fn test_collect_rust_files_skips_target_and_hidden() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_collect_{}", std::process::id()));
        fs::create_dir_all(temp_dir.join("src"))?;
        fs::create_dir_all(temp_dir.join("target"))?;
        fs::create_dir_all(temp_dir.join(".git"))?;

        fs::write(temp_dir.join("src/lib.rs"), "pub fn a() {}")?;
        fs::write(temp_dir.join("target/ignored.rs"), "pub fn b() {}")?;
        fs::write(temp_dir.join(".git/hook.rs"), "pub fn c() {}")?;

        let mut files = Vec::new();
        collect_rust_files(&temp_dir, &mut files)?;

        let _ = fs::remove_dir_all(&temp_dir);

        assert_that!(files.len(), eq(1));
        assert_that!(
            files[0].file_name().and_then(|s| s.to_str()),
            eq(Some("lib.rs"))
        );
        Ok(())
    }
}
