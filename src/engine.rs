use crate::cargo::{CargoManifest, LintConfig, RuleLevel};
use crate::diagnostics::{Diagnostic, DiagnosticReport, Severity, Span};
use crate::discovery::{discover_project_files, discover_project_files_from_manifest};
use crate::rules::default_rules;
use std::fs;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

/// Context provided to lint rules during AST traversal.
#[derive(Debug)]
pub struct LintContext<'a> {
    file_path: &'a Path,
    source: &'a str,
    lines: Vec<&'a str>,
    config: Option<&'a LintConfig>,
}

impl<'a> LintContext<'a> {
    /// Creates a new `LintContext` for the given file and source text.
    pub fn new(file_path: &'a Path, source: &'a str) -> Self {
        let lines = source.lines().collect();
        Self {
            file_path,
            source,
            lines,
            config: None,
        }
    }

    /// Sets the lint configuration for this context.
    pub fn with_config(mut self, config: &'a LintConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Returns the active lint configuration, if available.
    pub fn config(&self) -> Option<&'a LintConfig> {
        self.config
    }
    /// Returns the target file path.
    pub fn file_path(&self) -> &'a Path {
        self.file_path
    }

    /// Returns the entire source code of the file.
    pub fn source(&self) -> &'a str {
        self.source
    }

    /// Returns true if the current file is `main.rs` or `lib.rs`.
    pub fn is_main_or_lib(&self) -> bool {
        self.file_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name == "main.rs" || name == "lib.rs")
            .unwrap_or(false)
    }

    /// Returns true if the file is in a `tests/` directory or has a test suffix.
    pub fn is_test_file(&self) -> bool {
        self.file_path
            .components()
            .any(|c| c.as_os_str() == "tests")
            || self.file_path.ends_with("tests.rs")
            || self
                .file_path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("_test.rs"))
    }

    /// Converts a `proc_macro2::Span` into a `crate::diagnostics::Span`.
    pub fn to_span(&self, span: proc_macro2::Span) -> Span {
        let start = span.start();
        let end = span.end();
        Span::new(
            self.file_path,
            start.line,
            start.column.saturating_add(1),
            end.line,
            end.column.saturating_add(1),
        )
    }

    /// Checks if the line immediately preceding `line_number` contains a comment (`// ...`).
    /// Line numbers are 1-indexed.
    pub fn has_preceding_comment(&self, line_number: usize) -> bool {
        if line_number <= 1 {
            return false;
        }

        let prev_index = line_number.saturating_sub(2);
        self.lines.get(prev_index).is_some_and(|prev_line| {
            let trimmed = prev_line.trim_start();
            trimmed.starts_with("//") || trimmed.starts_with("/*")
        })
    }

    /// Returns the content of a specific line (1-indexed).
    pub fn line_content(&self, line_number: usize) -> Option<&'a str> {
        if line_number == 0 {
            return None;
        }
        self.lines.get(line_number.saturating_sub(1)).copied()
    }
}

/// Trait defining a purist static analysis rule.
pub trait Rule: Send + Sync {
    /// Returns the unique rule identifier (e.g. "purist::no_inline_mods").
    fn name(&self) -> &'static str;

    /// Analyzes the parsed AST file and reports any diagnostics found.
    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic>;
}

/// In-code suppression directive parsed from attributes.
#[derive(Debug, Clone)]
pub struct CodeSuppression {
    pub rule: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// Static analysis engine running registered purist rules over Rust source files.
pub struct PuristEngine {
    rules: Vec<Box<dyn Rule>>,
    config: Option<LintConfig>,
}

impl Default for PuristEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PuristEngine {
    /// Creates a new engine instance with no rules registered.
    pub fn empty() -> Self {
        Self {
            rules: Vec::new(),
            config: None,
        }
    }

    /// Creates a new engine instance with default purist rules registered.
    pub fn new() -> Self {
        Self {
            rules: default_rules(),
            config: None,
        }
    }

    /// Sets the project-level lint configuration.
    pub fn with_config(mut self, config: LintConfig) -> Self {
        self.config = Some(config);
        self
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
        self.check_source_internal(file_path, source, self.config.as_ref(), true)
    }

    fn check_source_internal(
        &self,
        file_path: &Path,
        source: &str,
        config: Option<&LintConfig>,
        emit_config_warnings: bool,
    ) -> DiagnosticReport {
        let mut report = DiagnosticReport::default();
        let mut ctx = LintContext::new(file_path, source);
        if let Some(cfg) = config {
            ctx = ctx.with_config(cfg);
        }

        match syn::parse_file(source) {
            Ok(ast) => {
                let suppressions = collect_code_suppressions(&ast, &ctx);
                let mut raw_diags = Vec::new();

                for rule in &self.rules {
                    // Skip rule if globally allowed in config
                    if let Some(cfg) = config
                        && cfg.level_for(rule.name()) == Some(RuleLevel::Allow)
                    {
                        continue;
                    }

                    raw_diags.extend(rule.check_file(&ctx, &ast));
                }

                for mut diag in raw_diags {
                    // Check in-code suppression (e.g. #[allow(purist::...)] or #[expect(...)])
                    if is_suppressed(&diag, &suppressions) {
                        continue;
                    }

                    // Apply config overrides (e.g. deny upgrades warning to error)
                    if let Some(cfg) = config {
                        match cfg.level_for(&diag.rule) {
                            Some(RuleLevel::Allow) => continue,
                            Some(RuleLevel::Deny | RuleLevel::Forbid) => {
                                diag.severity = Severity::Error;
                            }
                            Some(RuleLevel::Warn) => {
                                diag.severity = Severity::Warning;
                            }
                            None => {}
                        }
                    }

                    report.add(diag);
                }
            }
            Err(parse_err) => {
                let span = ctx.to_span(parse_err.span());
                report.add(
                    Diagnostic::new(
                        "purist::syntax_error",
                        Severity::Error,
                        format!("Failed to parse Rust syntax: {parse_err}"),
                    )
                    .with_span(span),
                );
            }
        }

        if emit_config_warnings && let Some(cfg) = config {
            for warning in cfg.warnings() {
                let rule = if warning.starts_with("Rule 'purist::")
                    || warning.starts_with("Rule 'opinionated::")
                {
                    "purist::deprecated_rule"
                } else {
                    "purist::unknown_rule"
                };
                report.add(
                    Diagnostic::new(rule, Severity::Warning, warning)
                        .with_span(Span::new(file_path, 1, 1, 1, 1)),
                );
            }
        }

        report
    }

    /// Analyzes a specific list of Rust files belonging to the project at `project_dir`
    /// using an explicitly provided `LintConfig`.
    pub fn check_files_with_config(
        &self,
        project_dir: &Path,
        files: &[PathBuf],
        config: &LintConfig,
    ) -> Result<DiagnosticReport, std::io::Error> {
        let mut report = DiagnosticReport::default();

        // Report manifest-level configuration warnings once for the target path
        let manifest_path = project_dir.join("Cargo.toml");
        for warning in config.warnings() {
            let rule = if warning.starts_with("Rule 'purist::")
                || warning.starts_with("Rule 'opinionated::")
            {
                "purist::deprecated_rule"
            } else {
                "purist::unknown_rule"
            };
            report.add(
                Diagnostic::new(rule, Severity::Warning, warning).with_span(Span::new(
                    &manifest_path,
                    1,
                    1,
                    1,
                    1,
                )),
            );
        }

        let mut targets_scanned = 0;
        for path in files {
            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(err) => {
                    report.add(
                        Diagnostic::new(
                            "purist::io_error",
                            Severity::Error,
                            format!("Failed to read file '{}': {err}", path.display()),
                        )
                        .with_span(Span::new(path, 1, 1, 1, 1)),
                    );
                    continue;
                }
            };

            targets_scanned += 1;
            let file_report = self.check_source_internal(path, &content, Some(config), false);
            for diag in file_report.diagnostics {
                report.add(diag);
            }
        }

        report.summary.targets_scanned = targets_scanned;
        Ok(report)
    }

    /// Analyzes a specific list of Rust files belonging to the project at `project_dir`.
    pub fn check_files(
        &self,
        project_dir: &Path,
        files: &[PathBuf],
    ) -> Result<DiagnosticReport, std::io::Error> {
        if let Some(cfg) = &self.config {
            self.check_files_with_config(project_dir, files, cfg)
        } else {
            let manifest = CargoManifest::load(project_dir).map_err(|e| match e {
                crate::PuristError::Io(err) => err,
                other => std::io::Error::other(other.to_string()),
            })?;
            self.check_files_with_config(project_dir, files, manifest.lint_config())
        }
    }

    /// Analyzes all Rust files in the specified path (file or directory).
    ///
    /// Requires a `Cargo.toml` in the directory (or enclosing workspace for a file).
    pub fn check_path(&self, target_path: &Path) -> Result<DiagnosticReport, std::io::Error> {
        if !target_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Path '{}' does not exist", target_path.display()),
            ));
        }

        let project_dir = if target_path.is_file() {
            target_path.parent().unwrap_or(target_path)
        } else {
            target_path
        };

        if let Some(cfg) = &self.config {
            let files = if target_path.is_file() {
                vec![target_path.to_path_buf()]
            } else {
                discover_project_files(project_dir).map_err(|e| match e {
                    crate::PuristError::Io(err) => err,
                    other => std::io::Error::other(other.to_string()),
                })?
            };
            self.check_files_with_config(project_dir, &files, cfg)
        } else {
            let manifest = CargoManifest::load(project_dir).map_err(|e| match e {
                crate::PuristError::Io(err) => err,
                other => std::io::Error::other(other.to_string()),
            })?;
            let files = if target_path.is_file() {
                vec![target_path.to_path_buf()]
            } else {
                discover_project_files_from_manifest(project_dir, &manifest).map_err(
                    |e| match e {
                        crate::PuristError::Io(err) => err,
                        other => std::io::Error::other(other.to_string()),
                    },
                )?
            };
            self.check_files_with_config(project_dir, &files, manifest.lint_config())
        }
    }
}

/// Collects in-code suppression attributes (`allow` and `expect`) and comment directives.
fn collect_code_suppressions(file: &syn::File, ctx: &LintContext<'_>) -> Vec<CodeSuppression> {
    let mut visitor = SuppressionVisitor {
        suppressions: Vec::new(),
    };
    visitor.visit_file(file);
    visitor
        .suppressions
        .extend(collect_comment_suppressions(ctx));
    visitor.suppressions
}

fn collect_comment_suppressions(ctx: &LintContext<'_>) -> Vec<CodeSuppression> {
    ctx.lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| parse_line_suppression(idx + 1, line))
        .collect()
}

fn parse_line_suppression(line_num: usize, line: &str) -> Option<CodeSuppression> {
    let trimmed = line.trim();

    if let Some(comment) = trimmed.strip_prefix("//!") {
        let rule = extract_directive_rule(comment.trim())?;
        return Some(CodeSuppression {
            rule,
            start_line: 1,
            end_line: usize::MAX,
        });
    }

    let idx = line.find("//")?;
    let comment = line.get(idx + 2..).map(str::trim).unwrap_or("");
    let rule = extract_directive_rule(comment)?;

    if trimmed.starts_with("//") {
        let is_file_allow = comment.starts_with("purist:file-allow")
            || comment.starts_with("opinionated:file-allow");
        let (start_line, end_line) = if is_file_allow {
            (1, usize::MAX)
        } else {
            (line_num + 1, line_num + 1)
        };
        Some(CodeSuppression {
            rule,
            start_line,
            end_line,
        })
    } else {
        Some(CodeSuppression {
            rule,
            start_line: line_num,
            end_line: line_num,
        })
    }
}

fn extract_directive_rule(comment: &str) -> Option<String> {
    for prefix in &[
        "purist:allow",
        "purist:disable",
        "purist:ignore",
        "purist:file-allow",
        "opinionated:allow",
        "opinionated:disable",
        "opinionated:ignore",
        "opinionated:file-allow",
    ] {
        if let Some(rest) = comment.strip_prefix(prefix) {
            let rest = rest.trim();
            if rest.starts_with('(') && rest.ends_with(')') {
                let inner = rest
                    .get(1..rest.len().saturating_sub(1))
                    .map(str::trim)
                    .unwrap_or("");
                let rule = inner
                    .strip_prefix("purist::")
                    .or_else(|| inner.strip_prefix("opinionated::"))
                    .unwrap_or(inner);
                return Some(if rule.is_empty() {
                    "all".to_string()
                } else {
                    rule.to_string()
                });
            } else if rest.is_empty() {
                return Some("all".to_string());
            }
        }
    }
    None
}

struct SuppressionVisitor {
    suppressions: Vec<CodeSuppression>,
}

impl SuppressionVisitor {
    fn check_attr(&mut self, attr: &syn::Attribute, start: usize, end: usize) {
        if !attr.path().is_ident("allow") && !attr.path().is_ident("expect") {
            return;
        }

        let _result = attr.parse_nested_meta(|meta| {
            let path_str = meta
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");

            if path_str == "purist" || path_str == "opinionated" {
                self.suppressions.push(CodeSuppression {
                    rule: "all".to_string(),
                    start_line: start,
                    end_line: end,
                });
            } else if let Some(rule_name) = path_str
                .strip_prefix("purist::")
                .or_else(|| path_str.strip_prefix("opinionated::"))
            {
                self.suppressions.push(CodeSuppression {
                    rule: rule_name.to_string(),
                    start_line: start,
                    end_line: end,
                });
            } else {
                self.suppressions.push(CodeSuppression {
                    rule: path_str,
                    start_line: start,
                    end_line: end,
                });
            }
            Ok(())
        });
    }
}

impl<'ast> Visit<'ast> for SuppressionVisitor {
    fn visit_file(&mut self, file: &'ast syn::File) {
        for attr in &file.attrs {
            self.check_attr(attr, 1, usize::MAX);
        }
        visit::visit_file(self, file);
    }

    fn visit_item(&mut self, item: &'ast syn::Item) {
        let span = item.span();
        let start = span.start().line;
        let end = span.end().line;
        for attr in get_item_attrs(item) {
            self.check_attr(attr, start, end);
        }
        visit::visit_item(self, item);
    }
}

fn get_item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Type(i) => &i.attrs,
        syn::Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

fn is_suppressed(diag: &Diagnostic, suppressions: &[CodeSuppression]) -> bool {
    let Some(span) = &diag.span else {
        return false;
    };
    let stripped = diag
        .rule
        .strip_prefix("purist::")
        .or_else(|| diag.rule.strip_prefix("opinionated::"))
        .unwrap_or(&diag.rule);

    for s in suppressions {
        if (s.rule == "all" || s.rule == diag.rule || s.rule == stripped)
            && span.start_line >= s.start_line
            && span.start_line <= s.end_line
        {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    struct DummyRule(&'static str);

    impl DummyRule {
        fn new() -> Self {
            Self("purist::dummy")
        }

        fn with_name(name: &'static str) -> Self {
            Self(name)
        }
    }

    impl Rule for DummyRule {
        fn name(&self) -> &'static str {
            self.0
        }

        fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
            let line = file
                .items
                .first()
                .map(|i| i.span().start().line)
                .unwrap_or(1);
            vec![
                Diagnostic::new(self.name(), Severity::Warning, "Dummy finding for testing")
                    .with_span(Span::new(ctx.file_path(), line, 1, line, 5)),
            ]
        }
    }

    #[googletest::test]
    fn lint_context_detects_entry_points_and_comments() -> googletest::Result<()> {
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
    fn lint_context_detects_test_file() -> googletest::Result<()> {
        let source = "fn helper() {}\n";
        let ctx = LintContext::new(Path::new("tests/integration_test.rs"), source);

        assert_that!(ctx.is_main_or_lib(), is_false());
        assert_that!(ctx.is_test_file(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn engine_check_source_syntax_error_produces_diagnostic()
    -> Result<(), Box<dyn std::error::Error>> {
        let engine = PuristEngine::empty();
        let report = engine.check_source(Path::new("bad.rs"), "fn broken syntax {{{");

        assert_that!(report.has_errors(), is_true());
        assert_that!(report.diagnostics.len(), eq(1));
        let diag = report.diagnostics.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::syntax_error"));
        Ok(())
    }

    #[googletest::test]
    fn engine_runs_registered_rule() -> Result<(), Box<dyn std::error::Error>> {
        let engine = PuristEngine::empty().with_rule(Box::new(DummyRule::new()));
        let report = engine.check_source(Path::new("clean.rs"), "fn ok() {}\n");

        assert_that!(report.has_errors(), is_false());
        assert_that!(report.warning_count(), eq(1));
        let diag = report.diagnostics.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::dummy"));
        Ok(())
    }

    #[googletest::test]
    fn in_code_suppression_via_allow_attribute() -> Result<(), Box<dyn std::error::Error>> {
        let engine = PuristEngine::empty().with_rule(Box::new(DummyRule::new()));
        let source = "#![allow(purist::dummy)]\nfn ok() {}\n";
        let report = engine.check_source(Path::new("clean.rs"), source);

        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn in_code_suppression_via_expect_attribute() -> Result<(), Box<dyn std::error::Error>> {
        let engine = PuristEngine::empty().with_rule(Box::new(DummyRule::new()));
        let source = "#![expect(dummy, reason = \"test\")]\nfn ok() {}\n";
        let report = engine.check_source(Path::new("clean.rs"), source);

        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn in_code_suppression_via_comment_next_line() -> Result<(), Box<dyn std::error::Error>> {
        let engine = PuristEngine::empty().with_rule(Box::new(DummyRule::new()));
        let source = "// purist:allow(dummy)\nfn ok() {}\n";
        let report = engine.check_source(Path::new("clean.rs"), source);

        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn in_code_suppression_via_comment_same_line() -> Result<(), Box<dyn std::error::Error>> {
        let engine = PuristEngine::empty().with_rule(Box::new(DummyRule::new()));
        let source = "fn ok() {} // purist:allow(dummy)\n";
        let report = engine.check_source(Path::new("clean.rs"), source);

        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn in_code_suppression_via_file_comment() -> Result<(), Box<dyn std::error::Error>> {
        let engine = PuristEngine::empty().with_rule(Box::new(DummyRule::new()));
        let source = "//! purist:allow(dummy)\nfn ok() {}\n";
        let report = engine.check_source(Path::new("clean.rs"), source);

        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn config_disables_rule() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = LintConfig::empty();
        config.set_rule("no_wildcard_imports", RuleLevel::Allow);
        let engine = PuristEngine::empty()
            .with_config(config)
            .with_rule(Box::new(DummyRule::with_name(
                "purist::no_wildcard_imports",
            )));
        let report = engine.check_source(Path::new("clean.rs"), "fn ok() {}\n");

        assert_that!(report.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn config_upgrades_rule_to_deny() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = LintConfig::empty();
        config.set_rule("purist::error_types", RuleLevel::Deny);
        let engine = PuristEngine::empty()
            .with_config(config)
            .with_rule(Box::new(DummyRule::with_name("purist::error_types")));
        let report = engine.check_source(Path::new("clean.rs"), "fn ok() {}\n");

        assert_that!(report.error_count(), eq(1));
        Ok(())
    }

    #[googletest::test]
    fn config_warns_on_unrecognized_rule() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = LintConfig::empty();
        config.set_rule("nonexistent_custom_rule", RuleLevel::Deny);
        let engine = PuristEngine::empty().with_config(config);
        let report = engine.check_source(Path::new("clean.rs"), "fn ok() {}\n");

        assert_that!(report.warning_count(), eq(1));
        let diag = report.diagnostics.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::unknown_rule"));
        Ok(())
    }

    #[googletest::test]
    fn config_warns_on_deprecated_rule() -> Result<(), Box<dyn std::error::Error>> {
        let mut config = LintConfig::empty();
        config
            .deprecated
            .push("Rule 'purist::old_rule' is deprecated. Use 'purist::new_rule' instead.".into());
        let engine = PuristEngine::empty().with_config(config);
        let report = engine.check_source(Path::new("clean.rs"), "fn ok() {}\n");

        assert_that!(report.warning_count(), eq(1));
        let diag = report.diagnostics.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("purist::deprecated_rule"));
        Ok(())
    }

    #[googletest::test]
    fn engine_default_registers_all_rules() -> Result<(), Box<dyn std::error::Error>> {
        let engine = PuristEngine::new();
        assert_that!(engine.rules().len(), eq(34));
        Ok(())
    }
}
