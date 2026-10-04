pub mod cargo_audit;
pub mod cargo_clippy;
pub mod cargo_fmt;
pub mod opinionated;
pub mod vcs_jj;

pub use cargo_audit::{AuditRunner, parse_audit_json};
pub use cargo_clippy::{ClippyRunner, parse_clippy_json_stream};
pub use cargo_fmt::{FmtRunner, parse_fmt_output};
pub use opinionated::OpinionatedRunner;
pub use vcs_jj::{JjError, JjVcs, filter_diagnostics_by_changed_files, parse_jj_diff_summary};

use code_review_diagnostics::{Diagnostic, DiagnosticReport};

/// Aggregates diagnostics from multiple checking tools into a consolidated `DiagnosticReport`.
pub fn aggregate_diagnostics(
    fmt_diags: Vec<Diagnostic>,
    clippy_diags: Vec<Diagnostic>,
    opinionated_report: DiagnosticReport,
    audit_diags: Vec<Diagnostic>,
) -> DiagnosticReport {
    let mut all = Vec::new();
    all.extend(fmt_diags);
    all.extend(clippy_diags);
    all.extend(opinionated_report.diagnostics);
    all.extend(audit_diags);
    DiagnosticReport::new(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use code_review_diagnostics::Severity;
    use googletest::prelude::*;

    #[googletest::test]
    fn aggregate_diagnostics_combines_all_sources_and_computes_summary()
    -> Result<(), Box<dyn std::error::Error>> {
        let fmt = vec![Diagnostic::new(
            "fmt::formatting",
            Severity::Warning,
            "bad fmt",
        )];
        let clippy = vec![Diagnostic::new(
            "clippy::foo",
            Severity::Error,
            "bad clippy",
        )];
        let op = DiagnosticReport::new(vec![Diagnostic::new(
            "opinionated::rule",
            Severity::Warning,
            "bad op",
        )]);
        let audit = vec![Diagnostic::new(
            "audit::vuln",
            Severity::Error,
            "vulnerability",
        )];

        let report = aggregate_diagnostics(fmt, clippy, op, audit);
        assert_that!(report.diagnostics.len(), eq(4));
        assert_that!(report.error_count(), eq(2));
        assert_that!(report.warning_count(), eq(2));
        assert_that!(report.has_errors(), is_true());
        Ok(())
    }
}
