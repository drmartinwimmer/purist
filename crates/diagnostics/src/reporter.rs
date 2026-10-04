use super::diagnostics::{Diagnostic, DiagnosticReport, Severity};
use annotate_snippets::{Group, Level, Origin, Renderer};
use anstyle::Style;
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
/// Automatically detects color support based on standard environment and terminal conventions.
pub fn render_report(
    report: &DiagnosticReport,
    format: OutputFormat,
    writer: &mut dyn io::Write,
) -> io::Result<()> {
    let use_color = auto_detect_color();
    render_report_with_options(report, format, writer, use_color)
}

fn auto_detect_color() -> bool {
    if anstyle_query::no_color() {
        return false;
    }
    if anstyle_query::clicolor_force() {
        return true;
    }
    if anstyle_query::clicolor() == Some(false) {
        return false;
    }
    true
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

    let renderer = if use_color {
        Renderer::styled()
    } else {
        Renderer::plain()
    };

    // Group diagnostics by file path (None represents global diagnostics)
    let mut by_file: BTreeMap<Option<&Path>, Vec<&Diagnostic>> = BTreeMap::new();
    for diag in &report.diagnostics {
        let file = diag.span.as_ref().map(|s| s.file.as_path());
        by_file.entry(file).or_default().push(diag);
    }

    for (_file_opt, diags) in by_file {
        for diag in diags {
            let level = match diag.severity {
                Severity::Error => Level::ERROR,
                Severity::Warning => Level::WARNING,
                Severity::Info => Level::INFO,
                Severity::Hint => Level::HELP,
            };

            let title = level.primary_title(&diag.message).id(&diag.rule);
            let mut group = Group::with_title(title);

            if let Some(span) = &diag.span {
                let path_str = span.file.display().to_string();
                let origin = Origin::path(path_str)
                    .line(span.start_line)
                    .char_column(span.start_col);
                group = group.element(origin);
            }

            if let Some(fix) = &diag.suggested_fix {
                group = group.element(Level::HELP.message(fix));
            }

            let rendered = renderer.render(&[group]);
            writeln!(writer, "{rendered}")?;
        }
    }

    let summary_line = format!(
        "{} error(s), {} warning(s), {} info, {} hint(s) found.",
        report.error_count(),
        report.warning_count(),
        report.info_count(),
        report.hint_count()
    );

    if use_color {
        let bold = Style::new().bold();
        writeln!(writer, "{bold}{summary_line}{bold:#}")?;
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
            Some(fix) => {
                let sanitized_fix = fix.replace('|', "\\|").replace('\n', " ");
                format!("`{sanitized_fix}`")
            }
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
    use crate::diagnostics::{Diagnostic, Severity, Span};
    use googletest::prelude::*;

    #[googletest::test]
    fn test_render_console_empty_report_reports_no_issues() -> Result<(), Box<dyn std::error::Error>>
    {
        let report = DiagnosticReport::default();
        let mut buffer = Vec::new();
        render_report_with_options(&report, OutputFormat::Console, &mut buffer, false)?;
        let output = String::from_utf8(buffer)?;
        expect_that!(output, contains_substring("No issues found."));
        Ok(())
    }

    #[googletest::test]
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

        expect_that!(output, contains_substring("--> src/main.rs:15:2"));
        expect_that!(output, contains_substring("--> src/main.rs:20:1"));
        expect_that!(output, contains_substring("error[rule::test]"));
        expect_that!(output, contains_substring("help[rule::hint]"));
        expect_that!(output, contains_substring("warning[rule::global]"));
        expect_that!(output, contains_substring("rule::test"));
        expect_that!(output, contains_substring("src/main.rs:15:2"));
        expect_that!(output, contains_substring("add semicolon"));
        expect_that!(output, contains_substring("1 error(s)"));
        expect_that!(output, contains_substring("1 warning(s)"));
        expect_that!(output, contains_substring("1 hint(s)"));
        Ok(())
    }

    #[googletest::test]
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

        expect_that!(output, contains_substring("\x1b["));
        expect_that!(output, contains_substring("\x1b[0m"));
        Ok(())
    }

    #[googletest::test]
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
        expect_that!(rule, eq(Some("rule::json")));

        let total_warnings = parsed
            .get("summary")
            .and_then(|s| s.get("total_warnings"))
            .and_then(|w| w.as_u64());
        expect_that!(total_warnings, eq(Some(1)));
        Ok(())
    }

    #[googletest::test]
    fn test_render_markdown_empty_report_outputs_clean_markdown()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = DiagnosticReport::default();
        let mut buffer = Vec::new();
        render_report(&report, OutputFormat::Markdown, &mut buffer)?;
        let output = String::from_utf8(buffer)?;
        expect_that!(output, contains_substring("# Diagnostic Report"));
        expect_that!(output, contains_substring("No issues found."));
        Ok(())
    }

    #[googletest::test]
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
        expect_that!(
            output,
            contains_substring("| Severity | Rule | Location | Message | Suggestion |")
        );
        expect_that!(
            output,
            contains_substring("| Error | `rule::md` | `lib.rs:1:1` | Missing doc | - |")
        );
        expect_that!(
            output,
            contains_substring("**Summary**: 1 error(s), 0 warning(s)")
        );
        Ok(())
    }

    #[googletest::test]
    fn test_render_markdown_sanitizes_pipes_and_newlines_in_suggested_fix()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut report = DiagnosticReport::default();
        report.add(
            Diagnostic::new("rule::fix", Severity::Warning, "Fix issue")
                .with_suggested_fix("first line | second line\nthird line"),
        );
        let mut buffer = Vec::new();
        render_report(&report, OutputFormat::Markdown, &mut buffer)?;
        let output = String::from_utf8(buffer)?;
        expect_that!(
            output,
            contains_substring("`first line \\| second line third line`")
        );
        Ok(())
    }

    #[googletest::test]
    fn render_report_renders_console_successfully() -> Result<(), Box<dyn std::error::Error>> {
        let mut report = DiagnosticReport::default();
        report.add(
            Diagnostic::new("rule::test", Severity::Warning, "Check code").with_span(Span::new(
                "src/lib.rs",
                10,
                5,
                10,
                15,
            )),
        );
        let mut buffer = Vec::new();
        render_report(&report, OutputFormat::Console, &mut buffer)?;
        let output = String::from_utf8(buffer)?;
        expect_that!(output, contains_substring("rule::test"));
        expect_that!(output, contains_substring("Check code"));
        expect_that!(output, contains_substring("src/lib.rs:10:5"));
        Ok(())
    }
}
