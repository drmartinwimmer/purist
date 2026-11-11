//! Strongly-typed rule identifiers and diagnostic findings for `purist`.
//!
//! Centralizes rule names (`RuleId`) and diagnostic enums (`ArchitectureDiagnostic`,
//! `HygieneDiagnostic`) using `thiserror::Error` and `miette::Diagnostic`.

mod architecture;
mod hygiene;
mod rule_id;

pub(crate) use self::architecture::ArchitectureDiagnostic;
pub(crate) use self::hygiene::HygieneDiagnostic;
pub(crate) use self::rule_id::RuleId;

use crate::diagnostics::{Diagnostic, Severity, Span};
use miette::{Diagnostic as MietteDiagnostic, Severity as MietteSeverity};

/// Extension trait converting any `miette::Diagnostic` + source [`Span`] into a report [`Diagnostic`].
pub(crate) trait IntoPuristDiagnostic: MietteDiagnostic + Sized {
    /// Converts this typed `miette::Diagnostic` and its source span into a [`Diagnostic`].
    fn into_diagnostic(self, span: Span) -> Diagnostic {
        let rule = self
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "purist::unknown".to_string());
        let severity = match self.severity() {
            Some(MietteSeverity::Error) => Severity::Error,
            Some(MietteSeverity::Warning) | None => Severity::Warning,
            Some(MietteSeverity::Advice) => Severity::Hint,
        };
        let mut diag = Diagnostic::new(rule, severity, self.to_string()).with_span(span);
        if let Some(help) = self.help() {
            diag = diag.with_suggested_fix(help.to_string());
        }
        diag
    }
}

impl<T: MietteDiagnostic> IntoPuristDiagnostic for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn rule_id_round_trips_unqualified_and_qualified_names() -> googletest::Result<()> {
        let id = RuleId::ErrorTypes;
        verify_that!(id.as_str(), eq("error_types"))?;
        verify_that!(id.qualified_name(), eq("purist::error_types"))?;
        verify_that!(RuleId::from_name("error_types"), some(eq(id)))?;
        verify_that!(RuleId::from_name("purist::error_types"), some(eq(id)))?;
        verify_that!(RuleId::from_name("opinionated::error_types"), some(eq(id)))?;
        Ok(())
    }

    #[googletest::test]
    fn rule_diagnostic_converts_into_populated_diagnostic() -> googletest::Result<()> {
        let span = Span::new(Path::new("src/lib.rs"), 10, 5, 10, 20);
        let diag = ArchitectureDiagnostic::BoxedDynErrorReturn {
            fn_name: "execute".to_string(),
        }
        .into_diagnostic(span);

        verify_that!(diag.rule, eq("purist::no_boxed_dyn_error"))?;
        verify_that!(diag.severity, eq(Severity::Error))?;
        verify_that!(
            diag.message,
            contains_substring("Function 'execute' returns 'Box<dyn Error>'")
        )?;
        verify_that!(
            diag.suggested_fix,
            some(contains_substring("thiserror::Error"))
        )?;
        Ok(())
    }
}
