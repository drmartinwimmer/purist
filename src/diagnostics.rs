use crate::rules::RuleDiagnostic;
pub use miette::{Diagnostic, Severity};
use miette::{LabeledSpan, MietteError, MietteSpanContents, SourceCode, SourceSpan, SpanContents};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

/// Source code location denoting start and end line/column coordinates, optional byte offsets,
/// and optional source code text for [`miette::SourceCode`] rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub file: PathBuf,
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) start_byte: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) end_byte: Option<usize>,
    #[serde(skip)]
    pub(crate) source: Option<Arc<str>>,
}

impl Span {
    /// Creates a new source code span without byte offsets.
    pub fn new(
        file: impl Into<PathBuf>,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Self {
        Self {
            file: file.into(),
            start_line,
            start_col,
            end_line,
            end_col,
            start_byte: None,
            end_byte: None,
            source: None,
        }
    }

    /// Attaches byte offsets to the span.
    pub fn with_byte_offsets(mut self, start_byte: usize, end_byte: usize) -> Self {
        self.start_byte = Some(start_byte);
        self.end_byte = Some(end_byte);
        self
    }

    /// Attaches shared source code text to the span.
    pub(crate) fn with_source(mut self, source: Arc<str>) -> Self {
        self.source = Some(source);
        self
    }
}

impl From<&Span> for SourceSpan {
    fn from(span: &Span) -> Self {
        if let (Some(start), Some(end)) = (span.start_byte, span.end_byte) {
            Self::new(start.into(), end.saturating_sub(start))
        } else {
            Self::new(0.into(), 0)
        }
    }
}

impl From<Span> for SourceSpan {
    fn from(span: Span) -> Self {
        Self::from(&span)
    }
}

impl SourceCode for Span {
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<Box<dyn SpanContents<'a> + 'a>, MietteError> {
        let name = self.file.display().to_string();
        if let Some(src) = &self.source
            && !src.is_empty()
            && self.start_byte.is_some()
            && span.offset() <= src.len()
            && let Ok(inner) =
                src.as_ref()
                    .read_span(span, context_lines_before, context_lines_after)
        {
            let contents = MietteSpanContents::new_named(
                name,
                inner.data(),
                *inner.span(),
                self.start_line.saturating_sub(1),
                self.start_col.saturating_sub(1),
                inner.line_count(),
            )
            .with_language("Rust");
            return Ok(Box::new(contents));
        }

        let line_count = self.end_line.saturating_sub(self.start_line).max(1);
        let contents = MietteSpanContents::new_named(
            name,
            &[],
            *span,
            self.start_line.saturating_sub(1),
            self.start_col.saturating_sub(1),
            line_count,
        );
        Ok(Box::new(contents))
    }
}

/// Engine-level diagnostics emitted during project parsing, I/O, or manifest configuration checks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, miette::Diagnostic)]
pub enum EngineDiagnostic {
    #[error("Failed to parse Rust syntax: {details}")]
    #[diagnostic(code(purist::syntax_error), severity(Error))]
    SyntaxError {
        details: String,
        #[source_code]
        #[label]
        span: Span,
    },

    #[error("Failed to read file '{}': {details}", path.display())]
    #[diagnostic(code(purist::io_error), severity(Error))]
    IoError {
        path: PathBuf,
        details: String,
        #[source_code]
        #[label]
        span: Span,
    },

    #[error("{message}")]
    #[diagnostic(code(purist::deprecated_rule), severity(Warning))]
    DeprecatedRule {
        message: String,
        #[source_code]
        #[label]
        span: Span,
    },

    #[error("{message}")]
    #[diagnostic(code(purist::unknown_rule), severity(Warning))]
    UnknownRule {
        message: String,
        #[source_code]
        #[label]
        span: Span,
    },
}

impl EngineDiagnostic {
    /// Returns the source [`Span`] associated with this engine diagnostic.
    pub const fn span(&self) -> &Span {
        match self {
            Self::SyntaxError { span, .. }
            | Self::IoError { span, .. }
            | Self::DeprecatedRule { span, .. }
            | Self::UnknownRule { span, .. } => span,
        }
    }
}

/// Top-level concrete diagnostic enum encompassing all rule and engine diagnostics produced by `purist`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PuristDiagnostic {
    /// A lint rule violation emitted by one of the built-in `purist` rules.
    #[error(transparent)]
    Rule(#[from] RuleDiagnostic),

    /// An engine-level syntax, I/O, or manifest configuration diagnostic.
    #[error(transparent)]
    Engine(#[from] EngineDiagnostic),

    /// A lint rule violation whose severity was overridden by `Cargo.toml` configuration.
    #[error("{diagnostic}")]
    Overridden {
        #[source]
        diagnostic: RuleDiagnostic,
        severity: Severity,
    },
}

impl PuristDiagnostic {
    /// Returns the source [`Span`] where this diagnostic was triggered.
    pub const fn span(&self) -> &Span {
        match self {
            Self::Rule(diag)
            | Self::Overridden {
                diagnostic: diag, ..
            } => diag.span(),
            Self::Engine(diag) => diag.span(),
        }
    }

    /// Overrides the [`Severity`] on a rule diagnostic according to `Cargo.toml` configuration.
    pub(crate) fn with_severity_override(diag: RuleDiagnostic, severity: Severity) -> Self {
        Self::Overridden {
            diagnostic: diag,
            severity,
        }
    }

    fn as_inner_diagnostic(&self) -> &dyn Diagnostic {
        match self {
            Self::Rule(diag)
            | Self::Overridden {
                diagnostic: diag, ..
            } => diag,
            Self::Engine(diag) => diag,
        }
    }
}

impl Diagnostic for PuristDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.as_inner_diagnostic().code()
    }

    fn severity(&self) -> Option<Severity> {
        match self {
            Self::Overridden { severity, .. } => Some(*severity),
            Self::Rule(diag) => diag.severity(),
            Self::Engine(diag) => diag.severity(),
        }
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.as_inner_diagnostic().help()
    }

    fn url<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.as_inner_diagnostic().url()
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.as_inner_diagnostic().source_code()
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        self.as_inner_diagnostic().labels()
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        self.as_inner_diagnostic().related()
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        self.as_inner_diagnostic().diagnostic_source()
    }
}

/// Summary metrics aggregated for a diagnostic report.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) struct ReportSummary {
    pub(crate) total_errors: usize,
    pub(crate) total_warnings: usize,
    pub(crate) total_advice: usize,
    pub(crate) targets_scanned: usize,
    pub(crate) duration_ms: u64,
}

#[derive(Serialize)]
struct SerializedDiagnostic<'a> {
    rule: String,
    severity: Severity,
    message: String,
    span: &'a Span,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_fix: Option<String>,
}

/// An aggregated container of all [`PuristDiagnostic`] findings produced during a check run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiagnosticReport {
    pub diagnostics: Vec<PuristDiagnostic>,
    pub(crate) summary: ReportSummary,
}

impl Serialize for DiagnosticReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let serialized_diags: Vec<SerializedDiagnostic<'_>> = self
            .diagnostics
            .iter()
            .map(|d| {
                let rule = d
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "purist::unknown".to_string());
                let severity = d.severity().unwrap_or(Severity::Warning);
                let suggested_fix = d.help().map(|h| h.to_string());
                SerializedDiagnostic {
                    rule,
                    severity,
                    message: d.to_string(),
                    span: d.span(),
                    suggested_fix,
                }
            })
            .collect();

        let mut state = serializer.serialize_struct("DiagnosticReport", 2)?;
        state.serialize_field("diagnostics", &serialized_diags)?;
        state.serialize_field("summary", &self.summary)?;
        state.end()
    }
}

impl DiagnosticReport {
    /// Creates a new diagnostic report with the given diagnostics, automatically computing summary counts.
    pub fn new(diagnostics: Vec<PuristDiagnostic>) -> Self {
        let mut total_errors = 0;
        let mut total_warnings = 0;
        let mut total_advice = 0;

        for d in &diagnostics {
            match d.severity().unwrap_or(Severity::Warning) {
                Severity::Error => total_errors += 1,
                Severity::Warning => total_warnings += 1,
                Severity::Advice => total_advice += 1,
            }
        }

        Self {
            diagnostics,
            summary: ReportSummary {
                total_errors,
                total_warnings,
                total_advice,
                targets_scanned: 0,
                duration_ms: 0,
            },
        }
    }

    /// Sets additional execution metrics on the summary.
    #[cfg(test)]
    pub(crate) fn with_metrics(mut self, targets_scanned: usize, duration_ms: u64) -> Self {
        self.summary.targets_scanned = targets_scanned;
        self.summary.duration_ms = duration_ms;
        self
    }

    /// Returns true if any diagnostic has error severity.
    pub fn has_errors(&self) -> bool {
        self.summary.total_errors > 0
    }

    /// Returns the total count of error diagnostics.
    pub fn error_count(&self) -> usize {
        self.summary.total_errors
    }

    /// Returns the total count of warning diagnostics.
    pub fn warning_count(&self) -> usize {
        self.summary.total_warnings
    }

    /// Returns the total count of advice/hint diagnostics.
    pub(crate) fn advice_count(&self) -> usize {
        self.summary.total_advice
    }

    /// Returns true if there are no diagnostics in the report.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Adds a diagnostic to the report and updates summary metrics.
    pub(crate) fn add(&mut self, diag: impl Into<PuristDiagnostic>) {
        let diag = diag.into();
        match diag.severity().unwrap_or(Severity::Warning) {
            Severity::Error => self.summary.total_errors += 1,
            Severity::Warning => self.summary.total_warnings += 1,
            Severity::Advice => self.summary.total_advice += 1,
        }
        self.diagnostics.push(diag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoWildcardImportsDiagnostic;
    use googletest::prelude::*;

    #[googletest::test]
    fn check_empty_report_returns_empty_and_zero_counts() -> Result<(), Box<dyn std::error::Error>>
    {
        let report = DiagnosticReport::default();
        expect_that!(report.is_empty(), is_true());
        expect_that!(report.has_errors(), is_false());
        expect_that!(report.error_count(), eq(0));
        expect_that!(report.warning_count(), eq(0));
        expect_that!(report.advice_count(), eq(0));
        expect_that!(report.summary.targets_scanned, eq(0));
        expect_that!(report.summary.duration_ms, eq(0));
        Ok(())
    }

    #[googletest::test]
    fn add_diagnostics_updates_error_and_warning_counts() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut report = DiagnosticReport::default();
        let span = Span::new("src/lib.rs", 1, 1, 1, 10);
        report.add(EngineDiagnostic::SyntaxError {
            details: "bad token".to_string(),
            span: span.clone(),
        });
        report.add(RuleDiagnostic::from(NoWildcardImportsDiagnostic {
            path_str: "foo::*".to_string(),
            span,
        }));

        expect_that!(report.is_empty(), is_false());
        expect_that!(report.has_errors(), is_true());
        expect_that!(report.error_count(), eq(1));
        expect_that!(report.warning_count(), eq(1));
        expect_that!(report.advice_count(), eq(0));
        expect_that!(report.diagnostics.len(), eq(2));
        expect_that!(report.summary.total_errors, eq(1));
        expect_that!(report.summary.total_warnings, eq(1));
        Ok(())
    }

    #[googletest::test]
    fn create_span_stores_coordinates_and_byte_offsets() -> Result<(), Box<dyn std::error::Error>> {
        let span = Span::new("src/lib.rs", 12, 4, 12, 18).with_byte_offsets(120, 134);
        expect_that!(span.file, eq(&PathBuf::from("src/lib.rs")));
        expect_that!(span.start_line, eq(12));
        expect_that!(span.start_col, eq(4));
        expect_that!(span.end_line, eq(12));
        expect_that!(span.end_col, eq(18));
        expect_that!(span.start_byte, eq(Some(120)));
        expect_that!(span.end_byte, eq(Some(134)));
        Ok(())
    }

    #[googletest::test]
    fn purist_diagnostic_exposes_span_and_miette_metadata() -> Result<(), Box<dyn std::error::Error>>
    {
        let span = Span::new("src/main.rs", 10, 5, 10, 20).with_byte_offsets(85, 100);
        let diag = PuristDiagnostic::from(RuleDiagnostic::from(NoWildcardImportsDiagnostic {
            path_str: "foo::*".to_string(),
            span: span.clone(),
        }));

        expect_that!(
            diag.code().map(|c| c.to_string()),
            some(eq("purist::no_wildcard_imports"))
        );
        expect_that!(
            diag.help().map(|h| h.to_string()),
            some(contains_substring("explicit item imports"))
        );
        expect_that!(diag.span(), eq(&span));
        Ok(())
    }

    #[googletest::test]
    fn serialize_report_with_summary_outputs_json() -> Result<(), Box<dyn std::error::Error>> {
        let span = Span::new("foo.rs", 1, 1, 1, 10);
        let report = DiagnosticReport::new(vec![
            PuristDiagnostic::from(EngineDiagnostic::SyntaxError {
                details: "unexpected eof".to_string(),
                span: span.clone(),
            }),
            PuristDiagnostic::with_severity_override(
                RuleDiagnostic::from(NoWildcardImportsDiagnostic {
                    path_str: "bar::*".to_string(),
                    span,
                }),
                Severity::Advice,
            ),
        ])
        .with_metrics(42, 150);

        let serialized = serde_json::to_value(&report)?;
        expect_that!(
            serialized
                .get("summary")
                .and_then(|s| s.get("total_errors"))
                .and_then(|v| v.as_u64()),
            some(eq(1))
        );
        expect_that!(
            serialized
                .get("summary")
                .and_then(|s| s.get("total_advice"))
                .and_then(|v| v.as_u64()),
            some(eq(1))
        );
        Ok(())
    }
}
