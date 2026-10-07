use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml_edit::{Item, Table, TomlError, value};

/// Profile defining which preset of Clippy lints to inject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LintProfile {
    /// Comprehensive safety, robustness, memory safety, numerics, and suppression hygiene rules (34 lints).
    #[default]
    Strict,
    /// Standard baseline safety and robustness rules (31 lints), omitting the strictest slicing
    /// and suppression bans (`string_slice`, `indexing_slicing`, and `allow_attributes`).
    Standard,
}

impl LintProfile {
    pub(crate) fn includes_lint(&self, lint: &str) -> bool {
        match self {
            Self::Strict => true,
            Self::Standard => !matches!(
                lint,
                "string_slice" | "indexing_slicing" | "allow_attributes"
            ),
        }
    }

    pub(crate) fn expected_count(&self) -> usize {
        match self {
            Self::Strict => 34,
            Self::Standard => 31,
        }
    }
}

/// Result metadata returned after modifying or inspecting a Cargo.toml file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigureResult {
    /// True if the file contents were modified on disk; false if already up-to-date or no lints removed.
    pub modified: bool,
    /// For `configure_lints`: The number of lints configured in the targeted table.
    /// For `remove_lints`: The number of lints removed from the targeted table (or 0 if none found).
    pub lints_configured: usize,
}

/// Errors that can occur during Cargo.toml inspection or modification.
#[derive(Debug, Error)]
pub enum CargoTomlError {
    #[error("Manifest file not found at path: {path}")]
    ManifestNotFound { path: PathBuf },

    #[error("I/O error accessing manifest file: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("Failed to parse TOML manifest: {source}")]
    Parse {
        #[from]
        source: TomlError,
    },

    /// Returned when an expected table (such as `[workspace]`, `[package]`, `[lints]`,
    /// or `[lints.clippy]`) exists in the document but is a scalar or array rather than
    /// a table or inline table.
    #[error("Invalid Cargo.toml structure: {reason}")]
    InvalidStructure { reason: String },
}

struct LintCategory {
    title: &'static str,
    description: &'static str,
    lints: &'static [&'static str],
}

const LINT_CATEGORIES: &[LintCategory] = &[
    LintCategory {
        title: "Don't Panic",
        description: "prevent panics from unwraps and unsafe slicing or indexing",
        lints: &[
            "string_slice",
            "indexing_slicing",
            "unwrap_used",
            "panic",
            "todo",
            "unimplemented",
            "unreachable",
            "get_unwrap",
            "unwrap_in_result",
            "unchecked_time_subtraction",
            "panic_in_result_fn",
        ],
    },
    LintCategory {
        title: "Don't Fail Silently",
        description: "prevent dropped futures and swallowed errors",
        lints: &[
            "let_underscore_future",
            "let_underscore_must_use",
            "unused_result_ok",
            "map_err_ignore",
            "assertions_on_result_states",
        ],
    },
    LintCategory {
        title: "Don't Do Unsafe Things with Memory",
        description: "",
        lints: &[
            "mem_forget",
            "undocumented_unsafe_blocks",
            "multiple_unsafe_ops_per_block",
            "unnecessary_safety_doc",
            "unnecessary_safety_comment",
        ],
    },
    LintCategory {
        title: "Don't Do Potentially Incorrect Things with Numbers",
        description: "",
        lints: &[
            "float_cmp",
            "float_cmp_const",
            "lossy_float_literal",
            "cast_sign_loss",
            "invalid_upcast_comparisons",
        ],
    },
    LintCategory {
        title: "Don't Do Bad Things That are Easy to Avoid",
        description: "",
        lints: &[
            "rc_mutex",
            "debug_assert_with_mut_call",
            "iter_not_returning_iterator",
            "expl_impl_clone_on_copy",
            "infallible_try_from",
            "dbg_macro",
        ],
    },
    LintCategory {
        title: "Don't `allow` Your Way Around These Lints",
        description: "every suppression must be a deliberate #[expect(..., reason = \"…\")] rather than a silent #[allow]",
        lints: &["allow_attributes", "allow_attributes_without_reason"],
    },
];

fn validate_document_structure(doc: &toml_edit::DocumentMut) -> Result<(), CargoTomlError> {
    if let Some(item) = doc.get("workspace") {
        let ws = item
            .as_table_like()
            .ok_or_else(|| CargoTomlError::InvalidStructure {
                reason: "'workspace' is not a table".to_string(),
            })?;
        if let Some(lints_item) = ws.get("lints") {
            let lints =
                lints_item
                    .as_table_like()
                    .ok_or_else(|| CargoTomlError::InvalidStructure {
                        reason: "'workspace.lints' is not a table".to_string(),
                    })?;
            if let Some(clippy_item) = lints.get("clippy")
                && clippy_item.as_table_like().is_none()
            {
                return Err(CargoTomlError::InvalidStructure {
                    reason: "'workspace.lints.clippy' is not a table".to_string(),
                });
            }
        }
    }

    if let Some(item) = doc.get("package") {
        let pkg = item
            .as_table_like()
            .ok_or_else(|| CargoTomlError::InvalidStructure {
                reason: "'package' is not a table".to_string(),
            })?;
        if pkg.get("lints").is_some() {
            return Err(CargoTomlError::InvalidStructure {
                reason: "'lints' must be at root level, not under [package]".to_string(),
            });
        }
    }

    if let Some(item) = doc.get("lints") {
        let lints = item
            .as_table_like()
            .ok_or_else(|| CargoTomlError::InvalidStructure {
                reason: "'lints' is not a table".to_string(),
            })?;
        if let Some(clippy_item) = lints.get("clippy")
            && clippy_item.as_table_like().is_none()
        {
            return Err(CargoTomlError::InvalidStructure {
                reason: "'lints.clippy' is not a table".to_string(),
            });
        }
    }

    Ok(())
}

fn populate_clippy_table(table: &mut toml_edit::Table, profile: LintProfile) {
    if table.is_empty() {
        let mut is_first_category = true;
        for category in LINT_CATEGORIES {
            let active_lints: Vec<&'static str> = category
                .lints
                .iter()
                .copied()
                .filter(|lint| profile.includes_lint(lint))
                .collect();

            if active_lints.is_empty() {
                continue;
            }

            let comment = if category.description.is_empty() {
                if is_first_category {
                    format!("# {}\n", category.title)
                } else {
                    format!("\n# {}\n", category.title)
                }
            } else if is_first_category {
                format!("# {} - {}\n", category.title, category.description)
            } else {
                format!("\n# {} - {}\n", category.title, category.description)
            };

            for (idx, lint) in active_lints.into_iter().enumerate() {
                let val = toml_edit::value("warn");
                table.insert(lint, val);
                if idx == 0
                    && let Some(mut k) = table.key_mut(lint)
                {
                    k.leaf_decor_mut().set_prefix(comment.as_str());
                }
            }

            is_first_category = false;
        }
    } else {
        // Table is not empty. If Standard profile, prune the 3 strict lints and transfer category comments.
        let mut transferred_panic_comment = None;
        let mut transferred_allow_comment = None;

        if profile == LintProfile::Standard {
            if let Some(k) = table.key("string_slice")
                && let Some(prefix) = k.leaf_decor().prefix().and_then(|p| p.as_str())
                && prefix.contains("# Don't Panic")
            {
                transferred_panic_comment = Some(prefix.to_string());
            }

            if let Some(k) = table.key("allow_attributes")
                && let Some(prefix) = k.leaf_decor().prefix().and_then(|p| p.as_str())
                && prefix.contains("# Don't `allow`")
            {
                transferred_allow_comment = Some(prefix.to_string());
            }

            table.remove("string_slice");
            table.remove("indexing_slicing");
            table.remove("allow_attributes");
        }

        for category in LINT_CATEGORIES {
            for lint in category.lints {
                if profile.includes_lint(lint) {
                    set_lint_to_warn(table, lint);
                }
            }
        }

        if let Some(comment) = transferred_panic_comment
            && let Some(mut k) = table.key_mut("unwrap_used")
            && k.leaf_decor()
                .prefix()
                .and_then(|p| p.as_str())
                .is_none_or(|s| !s.contains("# Don't Panic"))
        {
            k.leaf_decor_mut().set_prefix(comment);
        }

        if let Some(comment) = transferred_allow_comment
            && let Some(mut k) = table.key_mut("allow_attributes_without_reason")
            && k.leaf_decor()
                .prefix()
                .and_then(|p| p.as_str())
                .is_none_or(|s| !s.contains("# Don't `allow`"))
        {
            k.leaf_decor_mut().set_prefix(comment);
        }
    }
}

fn set_lint_to_warn(table: &mut Table, lint: &str) {
    if let Some(existing) = table.get_mut(lint) {
        if let Some(v) = existing.as_value_mut()
            && v.as_str() != Some("warn")
        {
            *existing = toml_edit::value("warn");
        }
    } else {
        table.insert(lint, toml_edit::value("warn"));
    }
}

/// Configures strict Clippy lints in the specified Cargo.toml manifest.
pub fn configure_lints(
    manifest_path: &Path,
    profile: LintProfile,
) -> Result<ConfigureResult, CargoTomlError> {
    if !manifest_path.exists() {
        return Err(CargoTomlError::ManifestNotFound {
            path: manifest_path.to_path_buf(),
        });
    }

    let original_raw = std::fs::read_to_string(manifest_path)?;
    let mut doc: toml_edit::DocumentMut = original_raw.parse()?;

    validate_document_structure(&doc)?;

    let has_workspace = doc.contains_key("workspace");
    let has_package = doc.contains_key("package");

    if !has_workspace && !has_package {
        return Err(CargoTomlError::InvalidStructure {
            reason: "Manifest contains neither [workspace] nor [package]".to_string(),
        });
    }

    if has_workspace {
        // Case 1 (Virtual workspace) or Case 2 (Root package with workspace)
        let ws = doc
            .get_mut("workspace")
            .and_then(|i| i.as_table_mut())
            .ok_or_else(|| CargoTomlError::InvalidStructure {
                reason: "'workspace' is not a table".to_string(),
            })?;

        if !ws.contains_key("lints") {
            let mut lints_table = Table::new();
            lints_table.set_implicit(true);
            ws.insert("lints", Item::Table(lints_table));
        }

        let ws_lints = ws
            .get_mut("lints")
            .and_then(|i| i.as_table_mut())
            .ok_or_else(|| CargoTomlError::InvalidStructure {
                reason: "'workspace.lints' is not a table".to_string(),
            })?;

        if !ws_lints.contains_key("clippy") {
            let mut clippy_table = Table::new();
            clippy_table.set_implicit(false);
            ws_lints.insert("clippy", Item::Table(clippy_table));
        }

        let clippy_table = ws_lints
            .get_mut("clippy")
            .and_then(|i| i.as_table_mut())
            .ok_or_else(|| CargoTomlError::InvalidStructure {
                reason: "'workspace.lints.clippy' is not a table".to_string(),
            })?;

        populate_clippy_table(clippy_table, profile);

        if has_package {
            // Case 2: Ensure [lints] workspace = true
            if !doc.contains_key("lints") {
                let mut lints_table = Table::new();
                lints_table.insert("workspace", value(true));
                doc.insert("lints", Item::Table(lints_table));
            } else {
                let lints_item =
                    doc.get_mut("lints")
                        .ok_or_else(|| CargoTomlError::InvalidStructure {
                            reason: "Missing lints table".to_string(),
                        })?;
                let lints_table =
                    lints_item
                        .as_table_mut()
                        .ok_or_else(|| CargoTomlError::InvalidStructure {
                            reason: "'lints' is not a table".to_string(),
                        })?;
                let is_ws_true = lints_table
                    .get("workspace")
                    .and_then(|v| v.as_value())
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !is_ws_true {
                    lints_table.insert("workspace", value(true));
                }
            }
        }
    } else {
        // Case 3: Single crate or member crate
        if !doc.contains_key("lints") {
            let mut lints_table = Table::new();
            lints_table.set_implicit(true);
            doc.insert("lints", Item::Table(lints_table));
        }

        let lints_table = doc
            .get_mut("lints")
            .and_then(|i| i.as_table_mut())
            .ok_or_else(|| CargoTomlError::InvalidStructure {
                reason: "'lints' is not a table".to_string(),
            })?;

        if !lints_table.contains_key("clippy") {
            let mut clippy_table = Table::new();
            clippy_table.set_implicit(false);
            lints_table.insert("clippy", Item::Table(clippy_table));
        }

        let clippy_table = lints_table
            .get_mut("clippy")
            .and_then(|i| i.as_table_mut())
            .ok_or_else(|| CargoTomlError::InvalidStructure {
                reason: "'lints.clippy' is not a table".to_string(),
            })?;

        populate_clippy_table(clippy_table, profile);
    }

    let new_raw = doc.to_string();
    let modified = new_raw != original_raw;

    if modified {
        std::fs::write(manifest_path, &new_raw)?;
    }

    Ok(ConfigureResult {
        modified,
        lints_configured: profile.expected_count(),
    })
}

/// Removes configured Clippy lints and associated tables from the specified Cargo.toml manifest.
pub fn remove_lints(manifest_path: &Path) -> Result<ConfigureResult, CargoTomlError> {
    if !manifest_path.exists() {
        return Err(CargoTomlError::ManifestNotFound {
            path: manifest_path.to_path_buf(),
        });
    }

    let original_raw = std::fs::read_to_string(manifest_path)?;
    let mut doc: toml_edit::DocumentMut = original_raw.parse()?;

    validate_document_structure(&doc)?;

    let mut removed_count = 0;

    // 1. Check [lints.clippy]
    if let Some(lints_item) = doc.get_mut("lints")
        && let Some(lints_table) = lints_item.as_table_mut()
    {
        if let Some(clippy_item) = lints_table.remove("clippy")
            && let Some(clippy_table) = clippy_item.as_table_like()
        {
            removed_count += clippy_table.iter().count();
        }
        if lints_table.is_empty() {
            doc.remove("lints");
        }
    }

    // 2. Check [workspace.lints.clippy]
    if let Some(ws_item) = doc.get_mut("workspace")
        && let Some(ws_table) = ws_item.as_table_mut()
        && let Some(ws_lints_item) = ws_table.get_mut("lints")
        && let Some(ws_lints_table) = ws_lints_item.as_table_mut()
    {
        if let Some(clippy_item) = ws_lints_table.remove("clippy")
            && let Some(clippy_table) = clippy_item.as_table_like()
        {
            removed_count += clippy_table.iter().count();
        }
        if ws_lints_table.is_empty() {
            ws_table.remove("lints");
        }
    }

    // 3. In Case 2 (Root package with workspace):
    // If [lints] only has `workspace = true` and [workspace.lints] was pruned, prune [lints]
    let ws_has_lints = doc
        .get("workspace")
        .and_then(|w| w.as_table_like())
        .is_some_and(|w| w.contains_key("lints"));

    if !ws_has_lints
        && let Some(lints_item) = doc.get_mut("lints")
        && let Some(lints_table) = lints_item.as_table_mut()
        && lints_table.len() == 1
        && lints_table.contains_key("workspace")
    {
        doc.remove("lints");
    }

    let new_raw = doc.to_string();
    let modified = new_raw != original_raw;

    if modified {
        std::fs::write(manifest_path, &new_raw)?;
    }

    Ok(ConfigureResult {
        modified,
        lints_configured: removed_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempManifest {
        path: PathBuf,
        dir: PathBuf,
    }

    impl TempManifest {
        fn new(content: &str) -> Result<Self, Box<dyn std::error::Error>> {
            let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "code_review_test_{}_{}",
                std::process::id(),
                id
            ));
            fs::create_dir_all(&dir)?;
            let path = dir.join("Cargo.toml");
            fs::write(&path, content)?;
            Ok(Self { path, dir })
        }

        fn read(&self) -> Result<String, Box<dyn std::error::Error>> {
            Ok(fs::read_to_string(&self.path)?)
        }
    }

    impl Drop for TempManifest {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.dir));
        }
    }

    #[googletest::test]
    fn configure_lints_with_comments_and_whitespace_preserves_formatting()
    -> Result<(), Box<dyn std::error::Error>> {
        let original = r#"# Top-level comment
[package]
name = "demo" # inline comment
version = "0.1.0"

# Dependency section comment
[dependencies]
serde = "1.0" # inline dep comment

# Trailing comment at end
"#;
        let manifest = TempManifest::new(original)?;
        let result = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(result.modified, is_true());
        expect_that!(result.lints_configured, eq(34));

        let modified_content = manifest.read()?;
        expect_that!(modified_content, contains_substring("# Top-level comment"));
        expect_that!(
            modified_content,
            contains_substring("name = \"demo\" # inline comment")
        );
        expect_that!(
            modified_content,
            contains_substring("# Dependency section comment")
        );
        expect_that!(
            modified_content,
            contains_substring("serde = \"1.0\" # inline dep comment")
        );
        expect_that!(
            modified_content,
            contains_substring("# Trailing comment at end")
        );
        expect_that!(modified_content, contains_substring("[lints.clippy]"));
        expect_that!(
            modified_content,
            contains_substring("unwrap_used = \"warn\"")
        );
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_twice_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "demo"
version = "0.1.0"
"#;
        let manifest = TempManifest::new(initial)?;
        let res1 = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(res1.modified, is_true());
        expect_that!(res1.lints_configured, eq(34));

        let content_after_run1 = manifest.read()?;

        let res2 = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(res2.modified, is_false());
        expect_that!(res2.lints_configured, eq(34));

        let content_after_run2 = manifest.read()?;
        expect_that!(content_after_run1, eq(&content_after_run2));
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_virtual_workspace_injects_only_workspace_clippy()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[workspace]
members = ["crates/*"]
"#;
        let manifest = TempManifest::new(initial)?;
        let res = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(res.modified, is_true());
        expect_that!(res.lints_configured, eq(34));

        let content = manifest.read()?;
        expect_that!(content, contains_substring("[workspace.lints.clippy]"));
        expect_that!(content, not(contains_substring("\n[lints]")));
        expect_that!(content, not(contains_substring("workspace = true")));
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_root_package_with_workspace_injects_workspace_clippy_and_workspace_true()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "root"
version = "0.1.0"

[workspace]
members = ["crates/*"]
"#;
        let manifest = TempManifest::new(initial)?;
        let res = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(res.modified, is_true());
        expect_that!(res.lints_configured, eq(34));

        let content = manifest.read()?;
        expect_that!(content, contains_substring("[workspace.lints.clippy]"));
        expect_that!(content, contains_substring("workspace = true"));
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_single_crate_injects_lints_clippy() -> Result<(), Box<dyn std::error::Error>>
    {
        let initial = r#"[package]
name = "single"
version = "0.1.0"
"#;
        let manifest = TempManifest::new(initial)?;
        let res = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(res.modified, is_true());
        expect_that!(res.lints_configured, eq(34));

        let content = manifest.read()?;
        expect_that!(content, contains_substring("[lints.clippy]"));
        expect_that!(content, not(contains_substring("[workspace")));
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_profile_differences_strict_34_and_standard_31()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "demo"
version = "0.1.0"
"#;
        let manifest_strict = TempManifest::new(initial)?;
        let res_strict = configure_lints(&manifest_strict.path, LintProfile::Strict)?;
        expect_that!(res_strict.lints_configured, eq(34));
        let content_strict = manifest_strict.read()?;
        expect_that!(
            content_strict,
            contains_substring("string_slice = \"warn\"")
        );
        expect_that!(
            content_strict,
            contains_substring("indexing_slicing = \"warn\"")
        );
        expect_that!(
            content_strict,
            contains_substring("allow_attributes = \"warn\"")
        );

        let manifest_standard = TempManifest::new(initial)?;
        let res_standard = configure_lints(&manifest_standard.path, LintProfile::Standard)?;
        expect_that!(res_standard.lints_configured, eq(31));
        let content_standard = manifest_standard.read()?;
        expect_that!(
            content_standard,
            not(contains_substring("string_slice = \"warn\""))
        );
        expect_that!(
            content_standard,
            not(contains_substring("indexing_slicing = \"warn\""))
        );
        expect_that!(
            content_standard,
            not(contains_substring("allow_attributes = \"warn\""))
        );
        expect_that!(
            content_standard,
            contains_substring("allow_attributes_without_reason = \"warn\"")
        );
        expect_that!(
            content_standard,
            contains_substring("unwrap_used = \"warn\"")
        );
        Ok(())
    }

    #[googletest::test]
    fn remove_lints_cleans_clippy_and_prunes_empty_parent_table()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "demo"
version = "0.1.0"
"#;
        let manifest = TempManifest::new(initial)?;
        configure_lints(&manifest.path, LintProfile::Strict)?;

        let remove_res = remove_lints(&manifest.path)?;
        expect_that!(remove_res.modified, is_true());
        expect_that!(remove_res.lints_configured, eq(34));

        let content_after_remove = manifest.read()?;
        expect_that!(content_after_remove, not(contains_substring("[lints]")));
        expect_that!(
            content_after_remove,
            not(contains_substring("[lints.clippy]"))
        );

        let remove_again = remove_lints(&manifest.path)?;
        expect_that!(remove_again.modified, is_false());
        expect_that!(remove_again.lints_configured, eq(0));
        Ok(())
    }

    #[googletest::test]
    fn remove_lints_with_existing_rust_lints_preserves_parent_table()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "demo"
version = "0.1.0"

[lints.rust]
unsafe_code = "forbid"
"#;
        let manifest = TempManifest::new(initial)?;
        configure_lints(&manifest.path, LintProfile::Strict)?;

        let content_configured = manifest.read()?;
        expect_that!(content_configured, contains_substring("[lints.rust]"));
        expect_that!(content_configured, contains_substring("[lints.clippy]"));

        let remove_res = remove_lints(&manifest.path)?;
        expect_that!(remove_res.modified, is_true());
        expect_that!(remove_res.lints_configured, eq(34));

        let content_after_remove = manifest.read()?;
        expect_that!(content_after_remove, contains_substring("[lints.rust]"));
        expect_that!(
            content_after_remove,
            contains_substring("unsafe_code = \"forbid\"")
        );
        expect_that!(
            content_after_remove,
            not(contains_substring("[lints.clippy]"))
        );
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_missing_file_returns_manifest_not_found()
    -> Result<(), Box<dyn std::error::Error>> {
        let missing = PathBuf::from("/non/existent/path/Cargo.toml");
        match configure_lints(&missing, LintProfile::Strict) {
            Err(CargoTomlError::ManifestNotFound { path }) => {
                expect_that!(path, eq(&missing));
            }
            other => return Err(format!("Expected ManifestNotFound, got {other:?}").into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_malformed_toml_returns_parse_error() -> Result<(), Box<dyn std::error::Error>>
    {
        let malformed = "this is not valid toml = [[";
        let manifest = TempManifest::new(malformed)?;
        match configure_lints(&manifest.path, LintProfile::Strict) {
            Err(CargoTomlError::Parse { .. }) => Ok(()),
            other => Err(format!("Expected Parse error, got {other:?}").into()),
        }
    }

    #[googletest::test]
    fn configure_lints_scalar_lints_or_workspace_returns_invalid_structure()
    -> Result<(), Box<dyn std::error::Error>> {
        let invalid_lints = r#"lints = "invalid_scalar"

[package]
name = "demo"
version = "0.1.0"
"#;
        let manifest1 = TempManifest::new(invalid_lints)?;
        match configure_lints(&manifest1.path, LintProfile::Strict) {
            Err(CargoTomlError::InvalidStructure { .. }) => {}
            other => {
                return Err(
                    format!("Expected InvalidStructure for scalar lints, got {other:?}").into(),
                );
            }
        }

        let invalid_ws = r#"workspace = 42
"#;
        let manifest2 = TempManifest::new(invalid_ws)?;
        match configure_lints(&manifest2.path, LintProfile::Strict) {
            Err(CargoTomlError::InvalidStructure { .. }) => {}
            other => {
                return Err(format!(
                    "Expected InvalidStructure for scalar workspace, got {other:?}"
                )
                .into());
            }
        }
        Ok(())
    }

    #[googletest::test]
    fn remove_lints_with_empty_clippy_table_prunes_empty_tables_on_disk()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "demo"
version = "0.1.0"

[lints.clippy]
"#;
        let manifest = TempManifest::new(initial)?;
        let remove_res = remove_lints(&manifest.path)?;
        expect_that!(remove_res.modified, is_true());
        expect_that!(remove_res.lints_configured, eq(0));

        let content_after = manifest.read()?;
        expect_that!(content_after, not(contains_substring("[lints]")));
        expect_that!(content_after, not(contains_substring("[lints.clippy]")));
        Ok(())
    }

    #[googletest::test]
    fn remove_lints_virtual_workspace_prunes_workspace_lints()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[workspace]
members = ["crates/*"]
"#;
        let manifest = TempManifest::new(initial)?;
        configure_lints(&manifest.path, LintProfile::Strict)?;
        let content_configured = manifest.read()?;
        expect_that!(
            content_configured,
            contains_substring("[workspace.lints.clippy]")
        );

        let remove_res = remove_lints(&manifest.path)?;
        expect_that!(remove_res.modified, is_true());
        expect_that!(remove_res.lints_configured, eq(34));

        let content_after = manifest.read()?;
        expect_that!(content_after, not(contains_substring("[workspace.lints]")));
        expect_that!(content_after, not(contains_substring("clippy")));
        expect_that!(content_after, contains_substring("[workspace]"));
        expect_that!(
            content_after,
            contains_substring("members = [\"crates/*\"]")
        );
        Ok(())
    }

    #[googletest::test]
    fn remove_lints_root_package_with_workspace_prunes_workspace_lints_and_root_lints()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "root"
version = "0.1.0"

[workspace]
members = ["crates/*"]
"#;
        let manifest = TempManifest::new(initial)?;
        configure_lints(&manifest.path, LintProfile::Strict)?;
        let content_configured = manifest.read()?;
        expect_that!(
            content_configured,
            contains_substring("[workspace.lints.clippy]")
        );
        expect_that!(content_configured, contains_substring("[lints]"));

        let remove_res = remove_lints(&manifest.path)?;
        expect_that!(remove_res.modified, is_true());
        expect_that!(remove_res.lints_configured, eq(34));

        let content_after = manifest.read()?;
        expect_that!(content_after, not(contains_substring("[workspace.lints]")));
        expect_that!(content_after, not(contains_substring("[lints]")));
        expect_that!(content_after, contains_substring("[package]"));
        expect_that!(content_after, contains_substring("[workspace]"));
        Ok(())
    }

    #[googletest::test]
    fn configure_lints_downgrade_to_standard_preserves_category_comments()
    -> Result<(), Box<dyn std::error::Error>> {
        let initial = r#"[package]
name = "demo"
version = "0.1.0"
"#;
        let manifest = TempManifest::new(initial)?;
        let res_strict = configure_lints(&manifest.path, LintProfile::Strict)?;
        expect_that!(res_strict.modified, is_true());
        let content_strict = manifest.read()?;
        expect_that!(content_strict, contains_substring("# Don't Panic"));
        expect_that!(content_strict, contains_substring("# Don't `allow`"));

        let res_standard = configure_lints(&manifest.path, LintProfile::Standard)?;
        expect_that!(res_standard.modified, is_true());
        expect_that!(res_standard.lints_configured, eq(31));

        let content_standard = manifest.read()?;
        expect_that!(content_standard, not(contains_substring("string_slice")));
        expect_that!(
            content_standard,
            not(contains_substring("allow_attributes ="))
        );
        expect_that!(content_standard, contains_substring("# Don't Panic"));
        expect_that!(content_standard, contains_substring("# Don't `allow`"));
        expect_that!(
            content_standard,
            contains_substring("unwrap_used = \"warn\"")
        );
        expect_that!(
            content_standard,
            contains_substring("allow_attributes_without_reason = \"warn\"")
        );
        Ok(())
    }
}
