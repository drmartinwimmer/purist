use code_review_diagnostics::{Diagnostic, Severity, Span};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Runner for `cargo audit --json`.
pub struct AuditRunner {
    target_path: PathBuf,
}

impl AuditRunner {
    /// Creates a new `AuditRunner` targeting the specified directory or workspace.
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Executes `cargo audit` and parses advisories into diagnostics.
    pub fn run(&self) -> Result<Vec<Diagnostic>, std::io::Error> {
        let lock_file = if self.target_path.is_file() {
            self.target_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("Cargo.lock")
        } else {
            self.target_path.join("Cargo.lock")
        };

        if !lock_file.exists() {
            return Ok(vec![Diagnostic::new(
                "audit::skipped",
                Severity::Info,
                "No Cargo.lock found in target directory; skipping security audit",
            )]);
        }

        // Try standard invocation first (with fetching enabled)
        let output = match self.invoke_audit(true) {
            Ok(out) => out,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(vec![Diagnostic::new(
                    "audit::skipped",
                    Severity::Info,
                    "cargo-audit binary is not installed; skipping dependency security scan",
                )]);
            }
            Err(err) => return Err(err),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // If fetch/lock failed, retry offline without fetching (--no-fetch)
        if !output.status.success() && stdout.trim().is_empty() {
            if let Ok(retry_output) = self.invoke_audit(false) {
                let retry_stdout = String::from_utf8_lossy(&retry_output.stdout);
                if !retry_stdout.trim().is_empty() {
                    return Ok(parse_audit_json(&retry_stdout, &lock_file));
                }
            }

            let error_msg = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                "cargo audit execution failed".to_string()
            };
            return Ok(vec![Diagnostic::new(
                "audit::error",
                Severity::Warning,
                error_msg,
            )]);
        }

        Ok(parse_audit_json(&stdout, &lock_file))
    }

    fn invoke_audit(&self, fetch: bool) -> Result<std::process::Output, std::io::Error> {
        let mut cmd = Command::new("cargo");
        cmd.arg("audit").arg("--json");

        if !fetch {
            cmd.arg("--no-fetch");
        }

        if self.target_path.is_dir() {
            cmd.current_dir(&self.target_path);
        } else if let Some(parent) = self.target_path.parent() {
            cmd.current_dir(parent);
        }

        cmd.output()
    }
}

#[derive(Debug, Deserialize, Default)]
struct AuditOutput {
    #[serde(default)]
    vulnerabilities: AuditVulnerabilities,
    #[serde(default)]
    warnings: BTreeMap<String, Vec<AuditWarningItem>>,
}

#[derive(Debug, Deserialize, Default)]
struct AuditVulnerabilities {
    #[serde(default)]
    list: Vec<AuditVulnerabilityItem>,
}

#[derive(Debug, Deserialize)]
struct AuditVulnerabilityItem {
    advisory: AuditAdvisory,
    package: AuditPackage,
    #[serde(default)]
    versions: Option<AuditVersions>,
}

#[derive(Debug, Deserialize)]
struct AuditAdvisory {
    id: String,
    title: String,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuditPackage {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct AuditVersions {
    #[serde(default)]
    patched: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AuditWarningItem {
    #[serde(default)]
    package: Option<AuditPackage>,
    #[serde(default)]
    advisory: Option<AuditAdvisory>,
}

/// Parses the JSON output of `cargo audit` into diagnostics.
pub fn parse_audit_json(json_str: &str, lock_file: &Path) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let trimmed = json_str.trim();

    if trimmed.is_empty() {
        return diagnostics;
    }

    let Ok(audit) = serde_json::from_str::<AuditOutput>(trimmed) else {
        return diagnostics;
    };

    let span = Span::new(lock_file.to_path_buf(), 1, 1, 1, 1);

    for vuln in audit.vulnerabilities.list {
        let title = &vuln.advisory.title;
        let id = &vuln.advisory.id;
        let pkg = format!("{}@{}", vuln.package.name, vuln.package.version);
        let msg = format!("Security vulnerability in {pkg}: {title} ({id})");

        let fix = if let Some(versions) = &vuln.versions {
            if !versions.patched.is_empty() {
                format!(
                    "Upgrade '{}' to {}",
                    vuln.package.name,
                    versions.patched.join(", ")
                )
            } else {
                format!(
                    "Consult advisory at {}",
                    vuln.advisory.url.as_deref().unwrap_or(id)
                )
            }
        } else {
            format!(
                "Consult advisory at {}",
                vuln.advisory.url.as_deref().unwrap_or(id)
            )
        };

        diagnostics.push(
            Diagnostic::new("audit::vulnerability", Severity::Error, msg)
                .with_span(span.clone())
                .with_suggested_fix(fix),
        );
    }

    for (kind, warnings) in audit.warnings {
        for warn in warnings {
            let pkg_name = warn
                .package
                .as_ref()
                .map(|p| format!("{}@{}", p.name, p.version))
                .unwrap_or_else(|| "dependency".to_string());

            let adv_id = warn
                .advisory
                .as_ref()
                .map(|a| a.id.as_str())
                .unwrap_or("notice");

            let title = warn
                .advisory
                .as_ref()
                .map(|a| a.title.as_str())
                .unwrap_or("dependency advisory notice");

            let msg = format!("Security warning [{kind}] for {pkg_name}: {title} ({adv_id})");

            diagnostics.push(
                Diagnostic::new(format!("audit::{kind}"), Severity::Warning, msg)
                    .with_span(span.clone()),
            );
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn parse_clean_audit_json_returns_empty_diagnostics() -> Result<(), Box<dyn std::error::Error>>
    {
        let json = r#"
{"database":{"advisory-count":1200},"lockfile":{"dependency-count":50},"settings":{},"vulnerabilities":{"found":false,"count":0,"list":[]},"warnings":{}}
"#;
        let diags = parse_audit_json(json, Path::new("Cargo.lock"));
        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn parse_vulnerability_audit_json_extracts_diagnostics_with_span_and_fix()
    -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"
{
  "vulnerabilities": {
    "found": true,
    "count": 1,
    "list": [
      {
        "advisory": {
          "id": "RUSTSEC-2020-0071",
          "title": "Potential segfault in time::parse",
          "url": "https://rustsec.org/advisories/RUSTSEC-2020-0071"
        },
        "package": {
          "name": "time",
          "version": "0.1.43"
        },
        "versions": {
          "patched": [">=0.2.23"]
        }
      }
    ]
  },
  "warnings": {}
}
"#;
        let diags = parse_audit_json(json, Path::new("Cargo.lock"));
        assert_that!(diags.len(), eq(1));

        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("audit::vulnerability"));
        assert_that!(diag.severity, eq(Severity::Error));
        assert_that!(
            &diag.message,
            contains_substring(
                "Security vulnerability in time@0.1.43: Potential segfault in time::parse (RUSTSEC-2020-0071)"
            )
        );
        assert_that!(
            diag.suggested_fix.as_deref(),
            eq(Some("Upgrade 'time' to >=0.2.23"))
        );

        let span = diag.span.as_ref().ok_or("expected span")?;
        assert_that!(span.file, eq(&PathBuf::from("Cargo.lock")));
        Ok(())
    }

    #[googletest::test]
    fn parse_unmaintained_warning_extracts_warning_diagnostic()
    -> Result<(), Box<dyn std::error::Error>> {
        let json = r#"
{
  "vulnerabilities": { "found": false, "count": 0, "list": [] },
  "warnings": {
    "unmaintained": [
      {
        "package": { "name": "net2", "version": "0.2.37" },
        "advisory": {
          "id": "RUSTSEC-2020-0016",
          "title": "net2 is unmaintained"
        }
      }
    ]
  }
}
"#;
        let diags = parse_audit_json(json, Path::new("Cargo.lock"));
        assert_that!(diags.len(), eq(1));

        let diag = diags.first().ok_or("expected diagnostic")?;
        assert_that!(&diag.rule, eq("audit::unmaintained"));
        assert_that!(diag.severity, eq(Severity::Warning));
        assert_that!(
            &diag.message,
            contains_substring(
                "Security warning [unmaintained] for net2@0.2.37: net2 is unmaintained (RUSTSEC-2020-0016)"
            )
        );
        Ok(())
    }
}
