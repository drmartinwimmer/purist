//! Project onboarding functionality for `purist`.
//!
//! Provides the `--onboard` capability to execute purist static analysis checks
//! and automatically disable any rules that trigger in `Cargo.toml`.

use crate::PuristError;
use crate::diagnostics::DiagnosticReport;
use crate::discovery::{find_cargo_toml, find_workspace_cargo_toml};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Table, value};

/// Map of deprecated rule aliases to their canonical rule names.
const DEPRECATED_ALIASES: &[(&str, &str)] = &[
    ("clippy_suppress", "clippy_suppression_hygiene"),
    ("use_declarations", "use_declarations_over_qualified_paths"),
    ("centralized_commands", "centralized_command_execution"),
    ("clap_encapsulation", "clap_struct_encapsulation"),
    ("option_bool_mapping", "idiomatic_option_bool_mapping"),
    ("test_matcher_borrow", "test_matcher_borrow_simplification"),
    ("no_negative_boolean_names", "no_double_negation"),
    ("no_negative_clap_flags", "no_negative_bool"),
];

/// Returns the canonical rule name if `rule_name` is a known purist lint rule.
pub fn canonical_rule_name(rule_name: &str) -> Option<&'static str> {
    let stripped = rule_name
        .strip_prefix("purist::")
        .or_else(|| rule_name.strip_prefix("opinionated::"))
        .unwrap_or(rule_name);

    let canonical = DEPRECATED_ALIASES
        .iter()
        .find(|(alias, _)| *alias == stripped)
        .map(|(_, can)| *can)
        .unwrap_or(stripped);

    match canonical {
        "no_inline_mods" => Some("no_inline_mods"),
        "free_functions" => Some("free_functions"),
        "path_resolution" => Some("path_resolution"),
        "error_types" => Some("error_types"),
        "clippy_suppression_hygiene" => Some("clippy_suppression_hygiene"),
        "test_patterns" => Some("test_patterns"),
        "no_redundant_conversions" => Some("no_redundant_conversions"),
        "use_declarations_over_qualified_paths" => Some("use_declarations_over_qualified_paths"),
        "no_redundant_wrappers" => Some("no_redundant_wrappers"),
        "no_boxed_dyn_error" => Some("no_boxed_dyn_error"),
        "test_matcher_borrow_simplification" => Some("test_matcher_borrow_simplification"),
        "no_test_prefix" => Some("no_test_prefix"),
        "no_unsafe_in_tests" => Some("no_unsafe_in_tests"),
        "centralized_command_execution" => Some("centralized_command_execution"),
        "clap_struct_encapsulation" => Some("clap_struct_encapsulation"),
        "exit_code_hygiene" => Some("exit_code_hygiene"),
        "idiomatic_option_bool_mapping" => Some("idiomatic_option_bool_mapping"),
        "no_wildcard_imports" => Some("no_wildcard_imports"),
        "no_env_access_outside_config" => Some("no_env_access_outside_config"),
        "single_match_to_let_else" => Some("single_match_to_let_else"),
        "raii_temp_directories" => Some("raii_temp_directories"),
        "no_println_in_libraries" => Some("no_println_in_libraries"),
        "cli_run_consumes_self" => Some("cli_run_consumes_self"),
        "no_double_negation" => Some("no_double_negation"),
        "googletest_conventions" => Some("googletest_conventions"),
        "max_file_lines" => Some("max_file_lines"),
        "no_trivial_getters_setters"
        | "no_trivial_getset"
        | "trivial_getters_setters"
        | "trivial_getset" => Some("no_trivial_getters_setters"),
        "max_nesting_depth" => Some("max_nesting_depth"),
        "lib_facade_hygiene" => Some("lib_facade_hygiene"),
        "no_negative_bool" => Some("no_negative_bool"),
        _ => None,
    }
}

/// Checks whether a manifest specifies `[lints] workspace = true`.
fn has_lints_workspace_true(manifest_path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(manifest_path) else {
        return false;
    };
    let Ok(doc) = content.parse::<DocumentMut>() else {
        return false;
    };
    doc.get("lints").is_some_and(|lints| {
        lints
            .as_table()
            .and_then(|t| t.get("workspace"))
            .and_then(|w| w.as_bool())
            == Some(true)
            || lints
                .as_inline_table()
                .and_then(|t| t.get("workspace"))
                .and_then(|w| w.as_bool())
                == Some(true)
    })
}

/// Checks whether the document defines `[workspace.lints.purist]` or `[workspace.lints.opinionated]`.
fn has_workspace_purist_table(doc: &DocumentMut) -> bool {
    doc.get("workspace")
        .and_then(|w| w.as_table())
        .and_then(|w| w.get("lints"))
        .and_then(|l| l.as_table())
        .is_some_and(|l| l.contains_key("purist") || l.contains_key("opinionated"))
}

/// Checks whether the existing table uses `opinionated` rather than `purist`.
fn has_opinionated_table(doc: &DocumentMut, in_workspace: bool) -> bool {
    if in_workspace {
        doc.get("workspace")
            .and_then(|w| w.as_table())
            .and_then(|w| w.get("lints"))
            .and_then(|l| l.as_table())
            .is_some_and(|l| l.contains_key("opinionated") && !l.contains_key("purist"))
    } else {
        doc.get("lints")
            .and_then(|l| l.as_table())
            .is_some_and(|l| l.contains_key("opinionated") && !l.contains_key("purist"))
    }
}

fn ensure_table_mut_path_2<'a>(
    doc: &'a mut DocumentMut,
    k1: &str,
    k2: &str,
) -> Result<&'a mut Table, PuristError> {
    if !doc.contains_key(k1) {
        let mut t1 = Table::new();
        t1.set_implicit(true);
        doc.insert(k1, Item::Table(t1));
    }
    let t1_item = doc
        .get_mut(k1)
        .and_then(|i| i.as_table_mut())
        .ok_or_else(|| PuristError::ManifestParse(format!("Expected [{k1}] to be a table")))?;

    if !t1_item.contains_key(k2) {
        let t2 = Table::new();
        t1_item.insert(k2, Item::Table(t2));
    }
    t1_item
        .get_mut(k2)
        .and_then(|i| i.as_table_mut())
        .ok_or_else(|| PuristError::ManifestParse(format!("Expected [{k1}.{k2}] to be a table")))
}

fn ensure_table_mut_path_3<'a>(
    doc: &'a mut DocumentMut,
    k1: &str,
    k2: &str,
    k3: &str,
) -> Result<&'a mut Table, PuristError> {
    if !doc.contains_key(k1) {
        let mut t1 = Table::new();
        t1.set_implicit(true);
        doc.insert(k1, Item::Table(t1));
    }
    let t1_item = doc
        .get_mut(k1)
        .and_then(|i| i.as_table_mut())
        .ok_or_else(|| PuristError::ManifestParse(format!("Expected [{k1}] to be a table")))?;

    if !t1_item.contains_key(k2) {
        let mut t2 = Table::new();
        t2.set_implicit(true);
        t1_item.insert(k2, Item::Table(t2));
    }
    let t2_item = t1_item
        .get_mut(k2)
        .and_then(|i| i.as_table_mut())
        .ok_or_else(|| PuristError::ManifestParse(format!("Expected [{k1}.{k2}] to be a table")))?;

    if !t2_item.contains_key(k3) {
        let t3 = Table::new();
        t2_item.insert(k3, Item::Table(t3));
    }
    t2_item
        .get_mut(k3)
        .and_then(|i| i.as_table_mut())
        .ok_or_else(|| {
            PuristError::ManifestParse(format!("Expected [{k1}.{k2}.{k3}] to be a table"))
        })
}

/// Disables the specified purist lint rules in the given `Cargo.toml` manifest file.
///
/// Returns the number of rules that were newly added or modified to `"allow"`.
pub fn disable_rules_in_manifest(
    manifest_path: &Path,
    rules: &[&str],
) -> Result<usize, PuristError> {
    if rules.is_empty() {
        return Ok(0);
    }

    let content = fs::read_to_string(manifest_path)?;
    let mut doc: DocumentMut = content
        .parse()
        .map_err(|e| PuristError::ManifestParse(format!("{e}")))?;

    let mut disabled_count = 0;

    let use_workspace_table = has_workspace_purist_table(&doc)
        || (doc.get("workspace").is_some() && doc.get("package").is_none());

    let sub_table_name = if has_opinionated_table(&doc, use_workspace_table) {
        "opinionated"
    } else {
        "purist"
    };

    let target_table = if use_workspace_table {
        ensure_table_mut_path_3(&mut doc, "workspace", "lints", sub_table_name)?
    } else {
        ensure_table_mut_path_2(&mut doc, "lints", sub_table_name)?
    };

    let mut sorted_rules: Vec<&str> = rules.to_vec();
    sorted_rules.sort_unstable();
    sorted_rules.dedup();

    for rule in sorted_rules {
        if let Some(item) = target_table.get_mut(rule) {
            if let Some(val_str) = item.as_str() {
                if val_str == "allow" {
                    continue;
                }
                *item = value("allow");
                disabled_count += 1;
            } else if let Some(inline) = item.as_inline_table_mut() {
                if inline.get("level").and_then(|l| l.as_str()) == Some("allow") {
                    continue;
                }
                inline.insert("level", "allow".into());
                disabled_count += 1;
            } else if let Some(tbl) = item.as_table_mut() {
                if tbl.get("level").and_then(|l| l.as_str()) == Some("allow") {
                    continue;
                }
                tbl.insert("level", value("allow"));
                disabled_count += 1;
            } else {
                *item = value("allow");
                disabled_count += 1;
            }
        } else {
            target_table.insert(rule, value("allow"));
            disabled_count += 1;
        }
    }

    if disabled_count > 0 {
        let mut output = doc.to_string();
        if !output.ends_with('\n') {
            output.push('\n');
        }
        fs::write(manifest_path, output)?;
    }

    Ok(disabled_count)
}

/// Disables purist lint rules that triggered in `report` across relevant `Cargo.toml` files.
///
/// Returns the total number of rules disabled.
pub fn onboard_project(
    target_path: &Path,
    report: &DiagnosticReport,
) -> Result<usize, PuristError> {
    let target_manifest = find_cargo_toml(target_path)
        .ok_or_else(|| PuristError::ManifestNotFound(target_path.to_path_buf()))?;

    let mut rules_by_manifest: BTreeMap<PathBuf, BTreeSet<&'static str>> = BTreeMap::new();

    for diag in &report.diagnostics {
        let Some(rule_name) = canonical_rule_name(&diag.rule) else {
            continue;
        };

        let file_manifest = diag
            .span
            .as_ref()
            .and_then(|s| find_cargo_toml(&s.file))
            .unwrap_or_else(|| target_manifest.clone());

        let final_manifest = if has_lints_workspace_true(&file_manifest) {
            find_workspace_cargo_toml(&file_manifest).unwrap_or(file_manifest)
        } else {
            file_manifest
        };

        rules_by_manifest
            .entry(final_manifest)
            .or_default()
            .insert(rule_name);
    }

    if rules_by_manifest.is_empty() {
        return Ok(0);
    }

    let mut total_disabled = 0;
    for (manifest_path, rules) in &rules_by_manifest {
        let rules_slice: Vec<&str> = rules.iter().copied().collect();
        total_disabled += disable_rules_in_manifest(manifest_path, &rules_slice)?;
    }

    Ok(total_disabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{Diagnostic, Severity, Span};
    use googletest::prelude::*;

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.0));
        }
    }

    #[googletest::test]
    fn canonical_rule_name_maps_canonical_and_deprecated_aliases_succeeds() {
        assert_that!(
            canonical_rule_name("purist::no_wildcard_imports"),
            eq(Some("no_wildcard_imports"))
        );
        assert_that!(
            canonical_rule_name("opinionated::no_wildcard_imports"),
            eq(Some("no_wildcard_imports"))
        );
        assert_that!(
            canonical_rule_name("no_wildcard_imports"),
            eq(Some("no_wildcard_imports"))
        );
        assert_that!(
            canonical_rule_name("purist::clippy_suppress"),
            eq(Some("clippy_suppression_hygiene"))
        );
        assert_that!(
            canonical_rule_name("clap_encapsulation"),
            eq(Some("clap_struct_encapsulation"))
        );
        assert_that!(canonical_rule_name("purist::syntax_error"), eq(None));
        assert_that!(canonical_rule_name("unknown_nonexistent_rule"), eq(None));
    }

    #[googletest::test]
    fn disable_rules_in_clean_manifest_creates_lints_purist_table_succeeds()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_onboard_clean_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let manifest_path = temp_dir.join("Cargo.toml");

        let initial = r#"[package]
name = "demo"
version = "0.1.0"
edition = "2024"
"#;
        fs::write(&manifest_path, initial)?;

        let disabled =
            disable_rules_in_manifest(&manifest_path, &["no_wildcard_imports", "error_types"])?;
        assert_that!(disabled, eq(2));

        let updated = fs::read_to_string(&manifest_path)?;
        assert_that!(updated, contains_substring("[lints.purist]"));
        assert_that!(updated, contains_substring("error_types = \"allow\""));
        assert_that!(
            updated,
            contains_substring("no_wildcard_imports = \"allow\"")
        );
        Ok(())
    }

    #[googletest::test]
    fn disable_rules_in_manifest_with_existing_clippy_lints_succeeds()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_onboard_clippy_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let manifest_path = temp_dir.join("Cargo.toml");

        let initial = r#"[package]
name = "demo"
version = "0.1.0"

[lints.clippy]
unwrap_used = "warn"
"#;
        fs::write(&manifest_path, initial)?;

        let disabled = disable_rules_in_manifest(&manifest_path, &["no_wildcard_imports"])?;
        assert_that!(disabled, eq(1));

        let updated = fs::read_to_string(&manifest_path)?;
        assert_that!(updated, contains_substring("[lints.clippy]"));
        assert_that!(updated, contains_substring("[lints.purist]"));
        assert_that!(
            updated,
            contains_substring("no_wildcard_imports = \"allow\"")
        );
        Ok(())
    }

    #[googletest::test]
    fn disable_rules_in_manifest_with_existing_purist_rules_updates_and_adds_succeeds()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_onboard_existing_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let manifest_path = temp_dir.join("Cargo.toml");

        let initial = r#"[package]
name = "demo"
version = "0.1.0"

[lints.purist]
free_functions = "deny"
no_inline_mods = "warn"
already_allowed = "allow"
"#;
        fs::write(&manifest_path, initial)?;

        let disabled = disable_rules_in_manifest(
            &manifest_path,
            &["no_inline_mods", "error_types", "already_allowed"],
        )?;
        // no_inline_mods updated (1), error_types added (1), already_allowed skipped (0)
        assert_that!(disabled, eq(2));

        let updated = fs::read_to_string(&manifest_path)?;
        assert_that!(updated, contains_substring("free_functions = \"deny\""));
        assert_that!(updated, contains_substring("no_inline_mods = \"allow\""));
        assert_that!(updated, contains_substring("error_types = \"allow\""));
        assert_that!(updated, contains_substring("already_allowed = \"allow\""));
        Ok(())
    }

    #[googletest::test]
    fn disable_rules_in_manifest_with_inline_table_updates_level_succeeds()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_onboard_inline_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let manifest_path = temp_dir.join("Cargo.toml");

        let initial = r#"[package]
name = "demo"
version = "0.1.0"

[lints.purist]
max_file_lines = { level = "warn", max_production_lines = 500 }
"#;
        fs::write(&manifest_path, initial)?;

        let disabled = disable_rules_in_manifest(&manifest_path, &["max_file_lines"])?;
        assert_that!(disabled, eq(1));

        let updated = fs::read_to_string(&manifest_path)?;
        assert_that!(
            updated,
            contains_substring(
                "max_file_lines = { level = \"allow\", max_production_lines = 500 }"
            )
        );
        Ok(())
    }

    #[googletest::test]
    fn disable_rules_in_manifest_with_subtable_updates_level_succeeds()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_onboard_subtable_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let manifest_path = temp_dir.join("Cargo.toml");

        let initial = r#"[package]
name = "demo"
version = "0.1.0"

[lints.purist.max_file_lines]
level = "deny"
max_production_lines = 400
"#;
        fs::write(&manifest_path, initial)?;

        let disabled = disable_rules_in_manifest(&manifest_path, &["max_file_lines"])?;
        assert_that!(disabled, eq(1));

        let updated = fs::read_to_string(&manifest_path)?;
        assert_that!(updated, contains_substring("level = \"allow\""));
        assert_that!(updated, contains_substring("max_production_lines = 400"));
        Ok(())
    }

    #[googletest::test]
    fn disable_rules_in_virtual_workspace_manifest_creates_workspace_lints_purist_succeeds()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_onboard_ws_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let manifest_path = temp_dir.join("Cargo.toml");

        let initial = r#"[workspace]
members = ["crates/*"]
"#;
        fs::write(&manifest_path, initial)?;

        let disabled = disable_rules_in_manifest(&manifest_path, &["no_wildcard_imports"])?;
        assert_that!(disabled, eq(1));

        let updated = fs::read_to_string(&manifest_path)?;
        assert_that!(updated, contains_substring("[workspace.lints.purist]"));
        assert_that!(
            updated,
            contains_substring("no_wildcard_imports = \"allow\"")
        );
        Ok(())
    }

    #[googletest::test]
    fn disable_rules_in_opinionated_manifest_preserves_opinionated_table_succeeds()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_onboard_op_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let manifest_path = temp_dir.join("Cargo.toml");

        let initial = r#"[package]
name = "legacy"
version = "0.1.0"

[lints.opinionated]
no_inline_mods = "warn"
"#;
        fs::write(&manifest_path, initial)?;

        let disabled =
            disable_rules_in_manifest(&manifest_path, &["no_inline_mods", "error_types"])?;
        assert_that!(disabled, eq(2));

        let updated = fs::read_to_string(&manifest_path)?;
        assert_that!(updated, contains_substring("[lints.opinionated]"));
        assert_that!(updated, contains_substring("no_inline_mods = \"allow\""));
        assert_that!(updated, contains_substring("error_types = \"allow\""));
        Ok(())
    }

    #[googletest::test]
    fn onboard_project_with_violations_disables_triggered_rules_succeeds()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_onboard_proj_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let manifest_path = temp_dir.join("Cargo.toml");

        let initial = r#"[package]
name = "proj"
version = "0.1.0"
edition = "2024"
"#;
        fs::write(&manifest_path, initial)?;

        let src_file = temp_dir.join("src/lib.rs");
        let mut report = DiagnosticReport::default();
        report.add(
            Diagnostic::new(
                "purist::no_wildcard_imports",
                Severity::Warning,
                "Wildcard import used",
            )
            .with_span(Span::new(&src_file, 1, 1, 1, 10)),
        );
        report.add(
            Diagnostic::new(
                "purist::error_types",
                Severity::Warning,
                "Unstructured error",
            )
            .with_span(Span::new(&src_file, 5, 1, 5, 20)),
        );

        let count = onboard_project(&temp_dir, &report)?;
        assert_that!(count, eq(2));

        let updated = fs::read_to_string(&manifest_path)?;
        assert_that!(updated, contains_substring("[lints.purist]"));
        assert_that!(updated, contains_substring("error_types = \"allow\""));
        assert_that!(
            updated,
            contains_substring("no_wildcard_imports = \"allow\"")
        );
        Ok(())
    }

    #[googletest::test]
    fn onboard_project_without_cargo_toml_returns_manifest_not_found()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_onboard_no_manifest_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        let report = DiagnosticReport::default();
        let result = onboard_project(&temp_dir, &report);
        match result {
            Err(PuristError::ManifestNotFound(p)) => {
                assert_that!(p, eq(&temp_dir));
            }
            other => return Err(format!("Expected ManifestNotFound, got {other:?}").into()),
        }
        Ok(())
    }

    #[googletest::test]
    fn onboard_project_with_clean_code_does_not_modify_manifest()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_onboard_clean_proj_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());
        let manifest_path = temp_dir.join("Cargo.toml");

        let initial = r#"[package]
name = "proj"
version = "0.1.0"
"#;
        fs::write(&manifest_path, initial)?;

        let report = DiagnosticReport::default();
        let count = onboard_project(&temp_dir, &report)?;
        assert_that!(count, eq(0));

        let content = fs::read_to_string(&manifest_path)?;
        assert_that!(content, eq(initial));
        Ok(())
    }
}
