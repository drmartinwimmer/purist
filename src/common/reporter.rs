use super::diagnostics::{Diagnostic, DiagnosticReport, Severity};
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

/// Output format for diagnostic reports.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Default,
    clap::ValueEnum,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Console,
    Json,
    Markdown,
}

/// Renders a diagnostic report to the specified writer in the chosen format.
/// Automatically detects color support based on the `NO_COLOR` environment variable.
pub fn render_report(
    report: &DiagnosticReport,
    format: OutputFormat,
    writer: &mut dyn io::Write,
) -> io::Result<()> {
    let use_color = std::env::var_os("NO_COLOR").is_none();
    render_report_with_options(report, format, writer, use_color)
}

/// Renders a diagnostic report with explicit control over ANSI colorization.
pub fn render_report_with_options(
    report: &DiagnosticReport,
    format: OutputFormat,
    writer: &mut dyn io::Write,
    use_color: bool,
) -> io::Result<()> {
    match format {
        OutputFormat::Console => render_console(report, writer, use_color),
        OutputFormat::Json => render_json(report, writer),
        OutputFormat::Markdown => render_markdown(report, writer),
    }
}

fn render_console(
    report: &DiagnosticReport,
    writer: &mut dyn io::Write,
    use_color: bool,
) -> io::Result<()> {
    if report.is_empty() {
        writeln!(writer, "No issues found.")?;
        return Ok(());
    }

    // Group diagnostics by file path (None represents global diagnostics)
    let mut by_file: BTreeMap<Option<&Path>, Vec<&Diagnostic>> = BTreeMap::new();
    for diag in &report.diagnostics {
        let file = diag.span.as_ref().map(|s| s.file.as_path());
        by_file.entry(file).or_default().push(diag);
    }

    for (file_opt, diags) in by_file {
        if let Some(file) = file_opt {
            if use_color {
                writeln!(writer, "\x1b[1m--> {}\x1b[0m", file.display())?;
            } else {
                writeln!(writer, "--> {}", file.display())?;
            }
        } else if use_color {
            writeln!(writer, "\x1b[1m--> (global)\x1b[0m")?;
        } else {
            writeln!(writer, "--> (global)")?;
        }

        for diag in diags {
            let (sev_str, sev_color) = match diag.severity {
                Severity::Error => ("error", "\x1b[1;31m"),
                Severity::Warning => ("warning", "\x1b[1;33m"),
                Severity::Info => ("info", "\x1b[1;36m"),
                Severity::Hint => ("hint", "\x1b[1;34m"),
            };

            if use_color {
                writeln!(
                    writer,
                    "  [{sev_color}{sev_str}\x1b[0m] \x1b[1m{}\x1b[0m: {}",
                    diag.rule, diag.message
                )?;
            } else {
                writeln!(writer, "  [{sev_str}] {}: {}", diag.rule, diag.message)?;
            }

            if let Some(span) = &diag.span {
                writeln!(
                    writer,
                    "    --> {}:{}:{}",
                    span.file.display(),
                    span.start_line,
                    span.start_col
                )?;
            }
            if let Some(fix) = &diag.suggested_fix {
                writeln!(writer, "    = help: {fix}")?;
            }
        }

        writeln!(writer)?;
    }

    let summary_line = format!(
        "{} error(s), {} warning(s), {} info, {} hint(s) found.",
        report.error_count(),
        report.warning_count(),
        report.info_count(),
        report.hint_count()
    );

    if use_color {
        writeln!(writer, "\x1b[1m{summary_line}\x1b[0m")?;
    } else {
        writeln!(writer, "{summary_line}")?;
    }

    Ok(())
}

fn render_json(report: &DiagnosticReport, writer: &mut dyn io::Write) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *writer, report).map_err(io::Error::other)?;
    writeln!(writer)?;
    Ok(())
}

fn render_markdown(report: &DiagnosticReport, writer: &mut dyn io::Write) -> io::Result<()> {
    writeln!(writer, "# Diagnostic Report\n")?;

    if report.is_empty() {
        writeln!(writer, "No issues found.")?;
        return Ok(());
    }

    writeln!(
        writer,
        "| Severity | Rule | Location | Message | Suggestion |"
    )?;
    writeln!(writer, "| --- | --- | --- | --- | --- |")?;

    for diag in &report.diagnostics {
        let location = match &diag.span {
            Some(s) => format!("`{}:{}:{}`", s.file.display(), s.start_line, s.start_col),
            None => "-".to_string(),
        };

        let suggestion = match &diag.suggested_fix {
            Some(fix) => format!("`{fix}`"),
            None => "-".to_string(),
        };

        let sanitized_msg = diag.message.replace('|', "\\|").replace('\n', " ");

        writeln!(
            writer,
            "| {:?} | `{}` | {} | {} | {} |",
            diag.severity, diag.rule, location, sanitized_msg, suggestion
        )?;
    }

    writeln!(
        writer,
        "\n**Summary**: {} error(s), {} warning(s), {} info, {} hint(s)",
        report.error_count(),
        report.warning_count(),
        report.info_count(),
        report.hint_count()
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::diagnostics::{Diagnostic, Severity, Span};

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
    fn test_render_console_empty_report_reports_no_issues() -> Result<(), Box<dyn std::error::Error>>
    {
        let report = DiagnosticReport::default();
        let mut buffer = Vec::new();
        render_report_with_options(&report, OutputFormat::Console, &mut buffer, false)?;
        let output = String::from_utf8(buffer)?;
        ensure!(
            output.contains("No issues found."),
            "Expected 'No issues found.' in output"
        );
        Ok(())
    }

    #[test]
    fn test_render_console_diagnostics_formats_grouped_by_file_and_summary()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut report = DiagnosticReport::default();
        report.add(
            Diagnostic::new("rule::test", Severity::Error, "Syntax error")
                .with_span(Span::new("src/main.rs", 15, 2, 15, 10))
                .with_suggested_fix("add semicolon"),
        );
        report.add(
            Diagnostic::new("rule::hint", Severity::Hint, "Consider refactoring")
                .with_span(Span::new("src/main.rs", 20, 1, 20, 5)),
        );
        report.add(Diagnostic::new(
            "rule::global",
            Severity::Warning,
            "Global issue",
        ));

        let mut buffer = Vec::new();
        render_report_with_options(&report, OutputFormat::Console, &mut buffer, false)?;
        let output = String::from_utf8(buffer)?;

        ensure!(output.contains("--> src/main.rs"), "Expected file header");
        ensure!(
            output.contains("--> (global)"),
            "Expected global group header"
        );
        ensure!(output.contains("[error]"), "Expected [error]");
        ensure!(output.contains("[hint]"), "Expected [hint]");
        ensure!(output.contains("[warning]"), "Expected [warning]");
        ensure!(output.contains("rule::test"), "Expected rule name");
        ensure!(
            output.contains("src/main.rs:15:2"),
            "Expected span location"
        );
        ensure!(output.contains("add semicolon"), "Expected suggestion");
        ensure!(output.contains("1 error(s)"), "Expected error count");
        ensure!(output.contains("1 warning(s)"), "Expected warning count");
        ensure!(output.contains("1 hint(s)"), "Expected hint count");
        Ok(())
    }

    #[test]
    fn test_render_console_with_color_includes_ansi_escape_codes()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut report = DiagnosticReport::default();
        report.add(
            Diagnostic::new("rule::err", Severity::Error, "Failure").with_span(Span::new(
                "src/lib.rs",
                1,
                1,
                1,
                2,
            )),
        );

        let mut buffer = Vec::new();
        render_report_with_options(&report, OutputFormat::Console, &mut buffer, true)?;
        let output = String::from_utf8(buffer)?;

        ensure!(
            output.contains("\x1b[1;31m"),
            "Expected red ANSI escape code for error"
        );
        ensure!(output.contains("\x1b[0m"), "Expected ANSI reset code");
        Ok(())
    }

    #[test]
    fn test_render_json_outputs_valid_pretty_json_with_summary()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut report = DiagnosticReport::default();
        report.add(Diagnostic::new(
            "rule::json",
            Severity::Warning,
            "Check format",
        ));
        let mut buffer = Vec::new();
        render_report(&report, OutputFormat::Json, &mut buffer)?;
        let parsed: serde_json::Value = serde_json::from_slice(&buffer)?;

        let rule = parsed
            .get("diagnostics")
            .and_then(|d| d.get(0))
            .and_then(|e| e.get("rule"))
            .and_then(|r| r.as_str());
        ensure_eq!(rule, Some("rule::json"));

        let total_warnings = parsed
            .get("summary")
            .and_then(|s| s.get("total_warnings"))
            .and_then(|w| w.as_u64());
        ensure_eq!(total_warnings, Some(1));
        Ok(())
    }

    #[test]
    fn test_render_markdown_empty_report_outputs_clean_markdown()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = DiagnosticReport::default();
        let mut buffer = Vec::new();
        render_report(&report, OutputFormat::Markdown, &mut buffer)?;
        let output = String::from_utf8(buffer)?;
        ensure!(output.contains("# Diagnostic Report"), "Expected title");
        ensure!(output.contains("No issues found."), "Expected clean status");
        Ok(())
    }

    #[test]
    fn test_render_markdown_diagnostics_outputs_table_and_summary()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut report = DiagnosticReport::default();
        report.add(
            Diagnostic::new("rule::md", Severity::Error, "Missing doc")
                .with_span(Span::new("lib.rs", 1, 1, 1, 5)),
        );
        let mut buffer = Vec::new();
        render_report(&report, OutputFormat::Markdown, &mut buffer)?;
        let output = String::from_utf8(buffer)?;
        ensure!(
            output.contains("| Severity | Rule | Location | Message | Suggestion |"),
            "Expected table header"
        );
        ensure!(
            output.contains("| Error | `rule::md` | `lib.rs:1:1` | Missing doc | - |"),
            "Expected row content"
        );
        ensure!(
            output.contains("**Summary**: 1 error(s), 0 warning(s)"),
            "Expected summary footer"
        );
        Ok(())
    }
}
