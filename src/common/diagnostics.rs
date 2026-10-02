use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Severity classification for a diagnostic finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Info => write!(f, "info"),
            Self::Hint => write!(f, "hint"),
        }
    }
}

/// Source code location denoting start and end line/column coordinates and optional byte offsets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub file: PathBuf,
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub start_byte: Option<usize>,
    pub end_byte: Option<usize>,
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
        }
    }

    /// Attaches byte offsets to the span.
    pub fn with_byte_offsets(mut self, start_byte: usize, end_byte: usize) -> Self {
        self.start_byte = Some(start_byte);
        self.end_byte = Some(end_byte);
        self
    }
}

/// An individual quality, safety, or style finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
    pub suggested_fix: Option<String>,
}

impl Diagnostic {
    /// Creates a new diagnostic without a span or suggested fix.
    pub fn new(rule: impl Into<String>, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            rule: rule.into(),
            severity,
            message: message.into(),
            span: None,
            suggested_fix: None,
        }
    }

    /// Attaches a source span to the diagnostic.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Attaches a suggested fix to the diagnostic.
    pub fn with_suggested_fix(mut self, fix: impl Into<String>) -> Self {
        self.suggested_fix = Some(fix.into());
        self
    }
}

/// Summary metrics aggregated for a diagnostic report.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReportSummary {
    pub total_errors: usize,
    pub total_warnings: usize,
    pub total_info: usize,
    pub total_hints: usize,
    pub targets_scanned: usize,
    pub duration_ms: u64,
}

/// An aggregated container of all diagnostics produced during a check run.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub diagnostics: Vec<Diagnostic>,
    pub summary: ReportSummary,
}

impl DiagnosticReport {
    /// Creates a new diagnostic report with the given diagnostics, automatically computing summary counts.
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        let mut total_errors = 0;
        let mut total_warnings = 0;
        let mut total_info = 0;
        let mut total_hints = 0;

        for d in &diagnostics {
            match d.severity {
                Severity::Error => total_errors += 1,
                Severity::Warning => total_warnings += 1,
                Severity::Info => total_info += 1,
                Severity::Hint => total_hints += 1,
            }
        }

        Self {
            diagnostics,
            summary: ReportSummary {
                total_errors,
                total_warnings,
                total_info,
                total_hints,
                targets_scanned: 0,
                duration_ms: 0,
            },
        }
    }

    /// Sets additional execution metrics on the summary.
    pub fn with_metrics(mut self, targets_scanned: usize, duration_ms: u64) -> Self {
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

    /// Returns the total count of informational diagnostics.
    pub fn info_count(&self) -> usize {
        self.summary.total_info
    }

    /// Returns the total count of hint diagnostics.
    pub fn hint_count(&self) -> usize {
        self.summary.total_hints
    }

    /// Returns true if there are no diagnostics in the report.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Adds a diagnostic to the report and updates summary metrics.
    pub fn add(&mut self, diag: Diagnostic) {
        match diag.severity {
            Severity::Error => self.summary.total_errors += 1,
            Severity::Warning => self.summary.total_warnings += 1,
            Severity::Info => self.summary.total_info += 1,
            Severity::Hint => self.summary.total_hints += 1,
        }
        self.diagnostics.push(diag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! ensure {
        ($cond:expr, $($arg:tt)*) => {
            if !$cond {
                return Err(format!($($arg)*).into());
            }
        };
    }

    macro_rules! ensure_eq {
        ($left:expr, $right:expr) => {
            if $left != $right {
                return Err(
                    format!("check failed: left: `{:?}`, right: `{:?}`", $left, $right).into(),
                );
            }
        };
    }

    #[test]
    fn test_check_empty_report_returns_empty_and_zero_counts()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = DiagnosticReport::default();
        ensure!(report.is_empty(), "expected empty report");
        ensure!(!report.has_errors(), "expected no errors");
        ensure_eq!(report.error_count(), 0);
        ensure_eq!(report.warning_count(), 0);
        ensure_eq!(report.info_count(), 0);
        ensure_eq!(report.hint_count(), 0);
        ensure_eq!(report.summary.targets_scanned, 0);
        ensure_eq!(report.summary.duration_ms, 0);
        Ok(())
    }

    #[test]
    fn test_add_diagnostics_updates_error_and_warning_counts()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut report = DiagnosticReport::default();
        report.add(Diagnostic::new(
            "rule::error",
            Severity::Error,
            "An error occurred",
        ));
        report.add(Diagnostic::new(
            "rule::warning",
            Severity::Warning,
            "A warning occurred",
        ));
        report.add(Diagnostic::new(
            "rule::info",
            Severity::Info,
            "Informational notice",
        ));
        report.add(Diagnostic::new(
            "rule::hint",
            Severity::Hint,
            "A helpful hint",
        ));

        ensure!(!report.is_empty(), "expected non-empty report");
        ensure!(report.has_errors(), "expected errors present");
        ensure_eq!(report.error_count(), 1);
        ensure_eq!(report.warning_count(), 1);
        ensure_eq!(report.info_count(), 1);
        ensure_eq!(report.hint_count(), 1);
        ensure_eq!(report.diagnostics.len(), 4);
        ensure_eq!(report.summary.total_errors, 1);
        ensure_eq!(report.summary.total_warnings, 1);
        ensure_eq!(report.summary.total_info, 1);
        ensure_eq!(report.summary.total_hints, 1);
        Ok(())
    }

    #[test]
    fn test_create_span_stores_coordinates_and_byte_offsets()
    -> Result<(), Box<dyn std::error::Error>> {
        let span = Span::new("src/lib.rs", 12, 4, 12, 18).with_byte_offsets(120, 134);
        ensure_eq!(span.file, PathBuf::from("src/lib.rs"));
        ensure_eq!(span.start_line, 12);
        ensure_eq!(span.start_col, 4);
        ensure_eq!(span.end_line, 12);
        ensure_eq!(span.end_col, 18);
        ensure_eq!(span.start_byte, Some(120));
        ensure_eq!(span.end_byte, Some(134));
        Ok(())
    }

    #[test]
    fn test_serialize_diagnostic_roundtrips_json() -> Result<(), Box<dyn std::error::Error>> {
        let span = Span::new("src/main.rs", 10, 5, 10, 20).with_byte_offsets(85, 100);
        let diag = Diagnostic::new("rule::style", Severity::Warning, "Avoid raw unwrap")
            .with_span(span.clone())
            .with_suggested_fix("use ? instead");

        let serialized = serde_json::to_string(&diag)?;
        let deserialized: Diagnostic = serde_json::from_str(&serialized)?;

        ensure_eq!(diag, deserialized);
        ensure_eq!(deserialized.span, Some(span.clone()));
        ensure_eq!(deserialized.suggested_fix.as_deref(), Some("use ? instead"));
        Ok(())
    }

    #[test]
    fn test_serialize_report_with_summary_roundtrips_json() -> Result<(), Box<dyn std::error::Error>>
    {
        let report = DiagnosticReport::new(vec![
            Diagnostic::new("rule::one", Severity::Error, "First issue")
                .with_span(Span::new("foo.rs", 1, 1, 1, 10)),
            Diagnostic::new("rule::two", Severity::Hint, "Second issue"),
        ])
        .with_metrics(42, 150);

        let serialized = serde_json::to_string_pretty(&report)?;
        let deserialized: DiagnosticReport = serde_json::from_str(&serialized)?;

        ensure_eq!(report, deserialized);
        ensure_eq!(deserialized.diagnostics.len(), 2);
        ensure_eq!(deserialized.summary.total_errors, 1);
        ensure_eq!(deserialized.summary.total_hints, 1);
        ensure_eq!(deserialized.summary.targets_scanned, 42);
        ensure_eq!(deserialized.summary.duration_ms, 150);
        Ok(())
    }

    #[test]
    fn test_display_severity_formats_lowercase() -> Result<(), Box<dyn std::error::Error>> {
        ensure_eq!(Severity::Error.to_string(), "error");
        ensure_eq!(Severity::Warning.to_string(), "warning");
        ensure_eq!(Severity::Info.to_string(), "info");
        ensure_eq!(Severity::Hint.to_string(), "hint");
        Ok(())
    }
}
