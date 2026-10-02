pub mod diagnostics;
pub mod reporter;

pub use diagnostics::{Diagnostic, DiagnosticReport, ReportSummary, Severity, Span};
pub use reporter::{OutputFormat, render_report, render_report_with_options};
