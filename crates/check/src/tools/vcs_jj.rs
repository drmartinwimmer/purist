use purist::{Diagnostic, DiagnosticReport};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Error type for Jujutsu VCS operations.
#[derive(Debug, thiserror::Error)]
pub enum JjError {
    #[error("Failed to execute 'jj --no-pager diff --summary': {0}")]
    Execution(#[source] std::io::Error),

    #[error("Jujutsu command exited with status {status}: {message}")]
    CommandFailed {
        status: std::process::ExitStatus,
        message: String,
    },
}

/// VCS interface for querying modified files in a Jujutsu repository.
pub struct JjVcs {
    repo_root: PathBuf,
}

impl JjVcs {
    /// Creates a new `JjVcs` instance pointing to the given repository root directory.
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    /// Queries `jj --no-pager diff --summary` and returns the set of modified or added files.
    pub fn query_changed_files(&self) -> Result<HashSet<PathBuf>, JjError> {
        let mut cmd = Command::new("jj");
        cmd.arg("--no-pager")
            .arg("diff")
            .arg("--summary")
            .current_dir(&self.repo_root);

        let output = cmd.output().map_err(JjError::Execution)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(JjError::CommandFailed {
                status: output.status,
                message: stderr.trim().to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parse_jj_diff_summary(&stdout, &self.repo_root))
    }
}

/// Parses the output of `jj --no-pager diff --summary` into a set of file paths.
pub fn parse_jj_diff_summary(output: &str, base_dir: &Path) -> HashSet<PathBuf> {
    let mut files = HashSet::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Summary format:
        // M path/to/file
        // A path/to/file
        // D path/to/file (deleted - ignored for linting)
        // R old/path -> new/path
        // C old/path -> new/path
        let mut parts = trimmed.split_whitespace();
        let Some(status) = parts.next() else {
            continue;
        };

        match status {
            "M" | "A" => {
                if let Some(path_str) = parts.next() {
                    insert_normalized_path(&mut files, path_str, base_dir);
                }
            }
            "R" | "C" => {
                // Find target path after "->"
                let rest: Vec<&str> = parts.collect();
                if let Some(pos) = rest.iter().position(|&s| s == "->")
                    && let Some(target_path) = rest.get(pos + 1)
                {
                    insert_normalized_path(&mut files, target_path, base_dir);
                }
            }
            // Deletions and other statuses do not produce modified files for linting
            _ => {}
        }
    }

    files
}

fn insert_normalized_path(set: &mut HashSet<PathBuf>, path_str: &str, base_dir: &Path) {
    let raw = PathBuf::from(path_str);
    set.insert(raw.clone());

    // Also insert relative to base_dir if base_dir is non-empty
    if base_dir != Path::new(".") && !base_dir.as_os_str().is_empty() {
        set.insert(base_dir.join(&raw));
    }
}

/// Filters diagnostics in a `DiagnosticReport` to only those referencing changed files.
/// Diagnostics without source code spans (e.g. workspace-wide errors) are preserved.
pub fn filter_diagnostics_by_changed_files(
    report: DiagnosticReport,
    changed_files: &HashSet<PathBuf>,
    base_dir: &Path,
) -> DiagnosticReport {
    if changed_files.is_empty() {
        // Retain only span-less diagnostics
        let spanless: Vec<Diagnostic> = report
            .diagnostics
            .into_iter()
            .filter(|d| d.span.is_none())
            .collect();
        return DiagnosticReport::new(spanless);
    }

    let filtered: Vec<Diagnostic> = report
        .diagnostics
        .into_iter()
        .filter(|diag| {
            let Some(span) = &diag.span else {
                return true;
            };

            let file = &span.file;

            // Direct match
            if changed_files.contains(file) {
                return true;
            }

            // Relative match against base_dir
            if let Ok(rel) = file.strip_prefix(base_dir)
                && changed_files.contains(rel)
            {
                return true;
            }

            // Match if any changed file ends with this file path
            for changed in changed_files {
                if file.ends_with(changed) || changed.ends_with(file) {
                    return true;
                }
            }

            false
        })
        .collect();

    DiagnosticReport::new(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use purist::{Severity, Span};

    #[googletest::test]
    fn parse_jj_diff_summary_extracts_modified_and_added_files()
    -> Result<(), Box<dyn std::error::Error>> {
        let diff = r#"
M src/lib.rs
A src/new_module.rs
D src/deleted.rs
R old_name.rs -> new_name.rs
C template.rs -> generated.rs
"#;
        let files = parse_jj_diff_summary(diff, Path::new("."));

        assert_that!(files.contains(&PathBuf::from("src/lib.rs")), is_true());
        assert_that!(
            files.contains(&PathBuf::from("src/new_module.rs")),
            is_true()
        );
        assert_that!(files.contains(&PathBuf::from("src/deleted.rs")), is_false());
        assert_that!(files.contains(&PathBuf::from("new_name.rs")), is_true());
        assert_that!(files.contains(&PathBuf::from("generated.rs")), is_true());
        Ok(())
    }

    #[googletest::test]
    fn parse_jj_diff_summary_empty_output_returns_empty_set()
    -> Result<(), Box<dyn std::error::Error>> {
        let files = parse_jj_diff_summary("", Path::new("."));
        assert_that!(files.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn filter_diagnostics_retains_changed_files_and_spanless_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut changed = HashSet::new();
        changed.insert(PathBuf::from("src/main.rs"));

        let report = DiagnosticReport::new(vec![
            Diagnostic::new("rule::keep", Severity::Warning, "in changed file")
                .with_span(Span::new("src/main.rs", 10, 1, 10, 1)),
            Diagnostic::new("rule::drop", Severity::Warning, "in unchanged file")
                .with_span(Span::new("src/other.rs", 5, 1, 5, 1)),
            Diagnostic::new("rule::global", Severity::Error, "workspace global"),
        ]);

        let filtered = filter_diagnostics_by_changed_files(report, &changed, Path::new("."));

        assert_that!(filtered.diagnostics.len(), eq(2));
        let first = filtered
            .diagnostics
            .first()
            .ok_or("expected first diagnostic")?;
        let second = filtered
            .diagnostics
            .get(1)
            .ok_or("expected second diagnostic")?;
        assert_that!(&first.rule, eq("rule::keep"));
        assert_that!(&second.rule, eq("rule::global"));
        assert_that!(filtered.warning_count(), eq(1));
        assert_that!(filtered.error_count(), eq(1));
        Ok(())
    }

    #[googletest::test]
    fn filter_diagnostics_with_empty_changed_set_drops_file_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let empty_changed = HashSet::new();
        let report = DiagnosticReport::new(vec![
            Diagnostic::new("rule::file", Severity::Warning, "file diagnostic")
                .with_span(Span::new("src/lib.rs", 1, 1, 1, 1)),
            Diagnostic::new("rule::global", Severity::Error, "global diagnostic"),
        ]);

        let filtered = filter_diagnostics_by_changed_files(report, &empty_changed, Path::new("."));
        assert_that!(filtered.diagnostics.len(), eq(1));
        let first = filtered.diagnostics.first().ok_or("expected diagnostic")?;
        assert_that!(&first.rule, eq("rule::global"));
        Ok(())
    }
}
