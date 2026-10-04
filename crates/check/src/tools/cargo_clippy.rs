use code_review_diagnostics::{Diagnostic, Severity, Span};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Runner for `cargo clippy --message-format=json`.
pub struct ClippyRunner {
    target_path: PathBuf,
}

impl ClippyRunner {
    /// Creates a new `ClippyRunner` targeting the specified directory or workspace.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes `cargo clippy` and parses compiler messages into diagnostics.
    pub fn run(&self) -> Result<Vec<Diagnostic>, std::io::Error> {
        let manifest_path = if self.target_path.is_file() {
            self.target_path.clone()
        } else {
            self.target_path.join("Cargo.toml")
        };

        let mut cmd = Command::new("cargo");
        cmd.arg("clippy")
            .arg("--all-targets")
            .arg("--all-features")
            .arg("--message-format=json");

        if manifest_path.exists() {
            cmd.arg("--manifest-path").arg(&manifest_path);
        } else {
            cmd.current_dir(&self.target_path);
        }

        let output = cmd.output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut diagnostics = parse_clippy_json_stream(&stdout, &self.target_path);

        if !output.status.success() && diagnostics.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let error_msg = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                "Clippy execution failed with non-zero exit code".to_string()
            };

            diagnostics.push(Diagnostic::new(
                "clippy::execution",
                Severity::Error,
                error_msg,
            ));
        }

        Ok(diagnostics)
    }
}

#[derive(Debug, Deserialize)]
struct RustcMessageEnvelope {
    reason: String,
    #[serde(default)]
    message: Option<RustcMessage>,
}

#[derive(Debug, Deserialize)]
struct RustcMessage {
    message: String,
    code: Option<RustcCode>,
    level: String,
    #[serde(default)]
    spans: Vec<RustcSpan>,
    #[serde(default)]
    children: Vec<RustcChildMessage>,
}

#[derive(Debug, Deserialize)]
struct RustcCode {
    code: String,
}

#[derive(Debug, Deserialize)]
struct RustcSpan {
    file_name: String,
    byte_start: usize,
    byte_end: usize,
    line_start: usize,
    line_end: usize,
    column_start: usize,
    column_end: usize,
    is_primary: bool,
    suggested_replacement: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RustcChildMessage {
    message: String,
    level: String,
    #[serde(default)]
    spans: Vec<RustcSpan>,
}

/// Parses a stream of newline-delimited JSON compiler messages into diagnostics.
pub fn parse_clippy_json_stream(stream: &str, target_dir: &Path) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for line in stream.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Ok(envelope) = serde_json::from_str::<RustcMessageEnvelope>(trimmed) else {
            continue;
        };

        if envelope.reason != "compiler-message" {
            continue;
        }

        let Some(msg) = envelope.message else {
            continue;
        };

        // Ignore compiler notes without actionable context (e.g. aborting due to previous error)
        if msg.level == "failure-note" {
            continue;
        }

        let severity = match msg.level.as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            "note" => Severity::Info,
            "help" => Severity::Hint,
            _ => Severity::Info,
        };

        let rule = msg
            .code
            .as_ref()
            .map(|c| c.code.clone())
            .unwrap_or_else(|| format!("rustc::{}", msg.level));

        // Find primary span or fallback to first available span
        let primary_span = msg
            .spans
            .iter()
            .find(|s| s.is_primary)
            .or_else(|| msg.spans.first());

        let span = primary_span.map(|s| {
            let raw_path = PathBuf::from(&s.file_name);
            let normalized_path = if raw_path.is_absolute() {
                if let Ok(rel) = raw_path.strip_prefix(target_dir) {
                    rel.to_path_buf()
                } else {
                    raw_path
                }
            } else {
                raw_path
            };

            Span::new(
                normalized_path,
                s.line_start,
                s.column_start,
                s.line_end,
                s.column_end,
            )
            .with_byte_offsets(s.byte_start, s.byte_end)
        });

        // Determine suggested fix from primary span replacement or child help messages
        let mut suggested_fix = primary_span
            .and_then(|s| s.suggested_replacement.clone())
            .filter(|r| !r.is_empty());

        if suggested_fix.is_none() {
            for child in &msg.children {
                if child.level == "help" {
                    if let Some(child_span) = child.spans.iter().find(|s| s.is_primary)
                        && let Some(replacement) = &child_span.suggested_replacement
                        && !replacement.is_empty()
                    {
                        suggested_fix = Some(replacement.clone());
                        break;
                    }
                    if suggested_fix.is_none() && !child.message.is_empty() {
                        suggested_fix = Some(child.message.clone());
                    }
                }
            }
        }

        let mut diag = Diagnostic::new(rule, severity, msg.message);
        if let Some(s) = span {
            diag = diag.with_span(s);
        }
        if let Some(fix) = suggested_fix {
            diag = diag.with_suggested_fix(fix);
        }

        diagnostics.push(diag);
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn parse_empty_stream_yields_no_diagnostics() -> Result<(), Box<dyn std::error::Error>> {
        let diags = parse_clippy_json_stream("", Path::new("."));
        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn parse_non_compiler_message_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"{"reason":"compiler-artifact","package_id":"foo 0.1.0","target":{}}"#;
        let diags = parse_clippy_json_stream(json, Path::new("."));
        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn parse_clippy_warning_maps_rule_code_span_and_suggestion()
    -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"
{"reason":"compiler-message","package_id":"demo 0.1.0","target":{"name":"demo"},"message":{"rendered":"warning: redundant clone\n","children":[{"children":[],"code":null,"level":"help","message":"remove this call","spans":[{"byte_end":50,"byte_start":40,"column_end":18,"column_start":8,"file_name":"src/main.rs","is_primary":true,"line_end":5,"line_start":5,"suggested_replacement":"","suggestion_applicability":"MachineApplicable","text":[]}]}],"code":{"code":"clippy::redundant_clone","explanation":null},"level":"warning","message":"redundant clone","spans":[{"byte_end":50,"byte_start":40,"column_end":18,"column_start":8,"file_name":"src/main.rs","is_primary":true,"line_end":5,"line_start":5,"suggested_replacement":null,"suggestion_applicability":null,"text":[]}]}}
"#;
        let diags = parse_clippy_json_stream(json, Path::new("."));
        assert_that!(diags.len(), eq(1));

        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("clippy::redundant_clone"));
        assert_that!(diag.severity, eq(Severity::Warning));
        assert_that!(&diag.message, eq("redundant clone"));

        let span = diag.span.as_ref().ok_or("expected span")?;
        assert_that!(span.file, eq(&PathBuf::from("src/main.rs")));
        assert_that!(span.start_line, eq(5));
        assert_that!(span.start_col, eq(8));
        assert_that!(span.end_line, eq(5));
        assert_that!(span.end_col, eq(18));

        assert_that!(diag.suggested_fix.as_deref(), eq(Some("remove this call")));
        Ok(())
    }

    #[googletest::test]
    fn parse_compiler_error_maps_error_severity() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"
{"reason":"compiler-message","message":{"code":{"code":"E0425"},"level":"error","message":"cannot find value `x` in this scope","spans":[{"byte_end":30,"byte_start":29,"column_end":6,"column_start":5,"file_name":"src/lib.rs","is_primary":true,"line_end":2,"line_start":2,"suggested_replacement":null}]}}
"#;
        let diags = parse_clippy_json_stream(json, Path::new("."));
        assert_that!(diags.len(), eq(1));

        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("E0425"));
        assert_that!(diag.severity, eq(Severity::Error));
        assert_that!(
            &diag.message,
            contains_substring("cannot find value `x` in this scope")
        );
        Ok(())
    }

    #[googletest::test]
    fn parse_failure_note_is_skipped() -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"
{"reason":"compiler-message","message":{"code":null,"level":"failure-note","message":"aborting due to 1 previous error","spans":[]}}
"#;
        let diags = parse_clippy_json_stream(json, Path::new("."));
        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }
}
