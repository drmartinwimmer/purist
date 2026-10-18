use crate::PuristError;
use crate::discovery::{find_cargo_toml, find_workspace_cargo_toml};
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::DocumentMut;

/// Severity configuration for a purist lint rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleLevel {
    Allow,
    #[default]
    Warn,
    Deny,
    Forbid,
}

impl RuleLevel {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "allow" | "off" | "disabled" => Some(RuleLevel::Allow),
            "warn" | "warning" => Some(RuleLevel::Warn),
            "deny" | "error" => Some(RuleLevel::Deny),
            "forbid" => Some(RuleLevel::Forbid),
            _ => None,
        }
    }
}

/// Strongly-typed struct holding configuration levels for all known purist lint rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PuristLintsConfig {
    pub(crate) no_inline_mods: RuleLevel,
    pub(crate) free_functions: RuleLevel,
    pub(crate) path_resolution: RuleLevel,
    pub(crate) error_types: RuleLevel,
    pub(crate) clippy_suppression_hygiene: RuleLevel,
    pub(crate) test_patterns: RuleLevel,
    pub(crate) no_redundant_conversions: RuleLevel,
    pub(crate) use_declarations_over_qualified_paths: RuleLevel,
    pub(crate) no_redundant_wrappers: RuleLevel,
    pub(crate) no_boxed_dyn_error: RuleLevel,
    pub(crate) test_matcher_borrow_simplification: RuleLevel,
    pub(crate) no_test_prefix: RuleLevel,
    pub(crate) no_unsafe_in_tests: RuleLevel,
    pub(crate) centralized_command_execution: RuleLevel,
    pub(crate) clap_struct_encapsulation: RuleLevel,
    pub(crate) command_struct_naming: RuleLevel,
    pub(crate) config_struct_naming: RuleLevel,
    pub(crate) exit_code_hygiene: RuleLevel,
    pub(crate) idiomatic_option_bool_mapping: RuleLevel,
    pub(crate) no_wildcard_imports: RuleLevel,
    pub(crate) no_env_access_outside_config: RuleLevel,
    pub(crate) single_match_to_let_else: RuleLevel,
    pub(crate) raii_temp_directories: RuleLevel,
    pub(crate) no_println_in_libraries: RuleLevel,
    pub(crate) cli_run_consumes_self: RuleLevel,
    pub(crate) no_double_negation: RuleLevel,
    pub(crate) googletest_conventions: RuleLevel,
    pub(crate) max_file_lines: RuleLevel,
    pub(crate) no_trivial_getters_setters: RuleLevel,
    pub(crate) max_nesting_depth: RuleLevel,
    pub(crate) lib_facade_hygiene: RuleLevel,
    pub(crate) no_negative_bool: RuleLevel,
    pub(crate) no_has_trait_prefix: RuleLevel,
    pub(crate) granular_visitor_hooks: RuleLevel,
    pub(crate) no_bifurcated_trait_dispatch: RuleLevel,
    pub(crate) no_primitive_flag_clutter: RuleLevel,
    pub(crate) no_interior_mutability_in_visitors: RuleLevel,
    pub(crate) prefer_scoped_closures: RuleLevel,
    pub(crate) descriptive_checker_names: RuleLevel,
}

impl PuristLintsConfig {
    pub(crate) fn get(&self, rule_name: &str) -> Option<RuleLevel> {
        let stripped = rule_name
            .strip_prefix("purist::")
            .or_else(|| rule_name.strip_prefix("opinionated::"))
            .unwrap_or(rule_name);
        match stripped {
            "no_inline_mods" => Some(self.no_inline_mods),
            "free_functions" => Some(self.free_functions),
            "path_resolution" => Some(self.path_resolution),
            "error_types" => Some(self.error_types),
            "clippy_suppression_hygiene" => Some(self.clippy_suppression_hygiene),
            "test_patterns" => Some(self.test_patterns),
            "no_redundant_conversions" => Some(self.no_redundant_conversions),
            "use_declarations_over_qualified_paths" => {
                Some(self.use_declarations_over_qualified_paths)
            }
            "no_redundant_wrappers" => Some(self.no_redundant_wrappers),
            "no_boxed_dyn_error" => Some(self.no_boxed_dyn_error),
            "test_matcher_borrow_simplification" => Some(self.test_matcher_borrow_simplification),
            "no_test_prefix" => Some(self.no_test_prefix),
            "no_unsafe_in_tests" => Some(self.no_unsafe_in_tests),
            "centralized_command_execution" => Some(self.centralized_command_execution),
            "clap_struct_encapsulation" => Some(self.clap_struct_encapsulation),
            "command_struct_naming" => Some(self.command_struct_naming),
            "config_struct_naming" => Some(self.config_struct_naming),
            "exit_code_hygiene" => Some(self.exit_code_hygiene),
            "idiomatic_option_bool_mapping" => Some(self.idiomatic_option_bool_mapping),
            "no_wildcard_imports" => Some(self.no_wildcard_imports),
            "no_env_access_outside_config" => Some(self.no_env_access_outside_config),
            "single_match_to_let_else" => Some(self.single_match_to_let_else),
            "raii_temp_directories" => Some(self.raii_temp_directories),
            "no_println_in_libraries" => Some(self.no_println_in_libraries),
            "cli_run_consumes_self" => Some(self.cli_run_consumes_self),
            "no_double_negation" => Some(self.no_double_negation),
            "googletest_conventions" => Some(self.googletest_conventions),
            "max_file_lines" => Some(self.max_file_lines),
            "no_trivial_getters_setters" => Some(self.no_trivial_getters_setters),
            "max_nesting_depth" => Some(self.max_nesting_depth),
            "lib_facade_hygiene" => Some(self.lib_facade_hygiene),
            "no_negative_bool" => Some(self.no_negative_bool),
            "no_has_trait_prefix" => Some(self.no_has_trait_prefix),
            "granular_visitor_hooks" => Some(self.granular_visitor_hooks),
            "no_bifurcated_trait_dispatch" => Some(self.no_bifurcated_trait_dispatch),
            "no_primitive_flag_clutter" => Some(self.no_primitive_flag_clutter),
            "no_interior_mutability_in_visitors" => Some(self.no_interior_mutability_in_visitors),
            "prefer_scoped_closures" => Some(self.prefer_scoped_closures),
            "descriptive_checker_names" => Some(self.descriptive_checker_names),
            _ => None,
        }
    }

    pub(crate) fn set(
        &mut self,
        rule_name: &str,
        level: RuleLevel,
    ) -> Result<(), UnrecognizedRule> {
        let stripped = rule_name
            .strip_prefix("purist::")
            .or_else(|| rule_name.strip_prefix("opinionated::"))
            .unwrap_or(rule_name);
        match stripped {
            "no_inline_mods" => self.no_inline_mods = level,
            "free_functions" => self.free_functions = level,
            "path_resolution" => self.path_resolution = level,
            "error_types" => self.error_types = level,
            "clippy_suppression_hygiene" => self.clippy_suppression_hygiene = level,
            "test_patterns" => self.test_patterns = level,
            "no_redundant_conversions" => self.no_redundant_conversions = level,
            "use_declarations_over_qualified_paths" => {
                self.use_declarations_over_qualified_paths = level
            }
            "no_redundant_wrappers" => self.no_redundant_wrappers = level,
            "no_boxed_dyn_error" => self.no_boxed_dyn_error = level,
            "test_matcher_borrow_simplification" => self.test_matcher_borrow_simplification = level,
            "no_test_prefix" => self.no_test_prefix = level,
            "no_unsafe_in_tests" => self.no_unsafe_in_tests = level,
            "centralized_command_execution" => self.centralized_command_execution = level,
            "clap_struct_encapsulation" => self.clap_struct_encapsulation = level,
            "command_struct_naming" => self.command_struct_naming = level,
            "config_struct_naming" => self.config_struct_naming = level,
            "exit_code_hygiene" => self.exit_code_hygiene = level,
            "idiomatic_option_bool_mapping" => self.idiomatic_option_bool_mapping = level,
            "no_wildcard_imports" => self.no_wildcard_imports = level,
            "no_env_access_outside_config" => self.no_env_access_outside_config = level,
            "single_match_to_let_else" => self.single_match_to_let_else = level,
            "raii_temp_directories" => self.raii_temp_directories = level,
            "no_println_in_libraries" => self.no_println_in_libraries = level,
            "cli_run_consumes_self" => self.cli_run_consumes_self = level,
            "no_double_negation" => self.no_double_negation = level,
            "googletest_conventions" => self.googletest_conventions = level,
            "max_file_lines" => self.max_file_lines = level,
            "no_trivial_getters_setters" => self.no_trivial_getters_setters = level,
            "max_nesting_depth" => self.max_nesting_depth = level,
            "lib_facade_hygiene" => self.lib_facade_hygiene = level,
            "no_negative_bool" => self.no_negative_bool = level,
            "no_has_trait_prefix" => self.no_has_trait_prefix = level,
            "granular_visitor_hooks" => self.granular_visitor_hooks = level,
            "no_bifurcated_trait_dispatch" => self.no_bifurcated_trait_dispatch = level,
            "no_primitive_flag_clutter" => self.no_primitive_flag_clutter = level,
            "no_interior_mutability_in_visitors" => self.no_interior_mutability_in_visitors = level,
            "prefer_scoped_closures" => self.prefer_scoped_closures = level,
            "descriptive_checker_names" => self.descriptive_checker_names = level,
            _ => return Err(UnrecognizedRule),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnrecognizedRule;

pub(crate) use crate::rule_config::{
    LibFacadeHygieneConfig, MaxFileLinesConfig, MaxNestingDepthConfig,
};

/// Project-level lint configuration parsed from `Cargo.toml`.
#[derive(Debug, Clone, Default)]
pub struct LintConfig {
    pub(crate) rules: PuristLintsConfig,
    pub(crate) unrecognized: Vec<String>,
    pub(crate) deprecated: Vec<String>,
    pub(crate) invalid_values: Vec<String>,
    pub(crate) max_file_lines: MaxFileLinesConfig,
    pub(crate) max_nesting_depth: MaxNestingDepthConfig,
    pub(crate) lib_facade_hygiene: LibFacadeHygieneConfig,
}

impl LintConfig {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn set_rule(&mut self, rule: impl Into<String>, level: RuleLevel) {
        let name = rule.into();
        let stripped = name
            .strip_prefix("purist::")
            .or_else(|| name.strip_prefix("opinionated::"))
            .unwrap_or(&name);

        if self.rules.set(stripped, level).is_err() {
            self.unrecognized.push(stripped.to_string());
        }
    }

    pub fn level_for(&self, rule_name: &str) -> Option<RuleLevel> {
        self.rules.get(rule_name)
    }

    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        for dep in &self.deprecated {
            warnings.push(dep.clone());
        }
        for unrec in &self.unrecognized {
            warnings.push(format!(
                "Unrecognized purist lint rule '{unrec}' specified in Cargo.toml."
            ));
        }
        for inv in &self.invalid_values {
            warnings.push(inv.clone());
        }
        warnings
    }

    /// Loads lint configuration from the nearest `Cargo.toml` at or above `path`,
    /// inheriting workspace defaults if within a Cargo workspace.
    pub fn discover_for_path(path: &Path) -> Self {
        if let Some(cargo_toml) = find_cargo_toml(path) {
            CargoManifest::load(&cargo_toml)
                .map(CargoManifest::into_lint_config)
                .unwrap_or_default()
        } else {
            Self::empty()
        }
    }

    /// Parses lint configuration from a `Cargo.toml` file content.
    pub fn from_manifest_content(content: &str) -> Option<Self> {
        let doc: DocumentMut = content.parse().ok()?;
        Some(Self::from_document(&doc))
    }

    /// Parses lint configuration from a parsed `Cargo.toml` document.
    pub(crate) fn from_document(doc: &DocumentMut) -> Self {
        let mut config = Self::empty();

        // 1. Check [lints.purist] or [workspace.lints.purist] (with [lints.opinionated] fallback)
        if let Some(lints) = doc.get("lints").and_then(|l| l.as_table())
            && let Some(table) = lints
                .get("purist")
                .or_else(|| lints.get("opinionated"))
                .and_then(|o| o.as_table())
        {
            config.parse_rules_table(table);
        }

        if let Some(ws) = doc.get("workspace").and_then(|w| w.as_table())
            && let Some(lints) = ws.get("lints").and_then(|l| l.as_table())
            && let Some(table) = lints
                .get("purist")
                .or_else(|| lints.get("opinionated"))
                .and_then(|o| o.as_table())
        {
            config.parse_rules_table(table);
        }

        // 2. Check [package.metadata.purist] or [workspace.metadata.purist]
        // (with metadata.opinionated fallback)
        if let Some(pkg) = doc.get("package").and_then(|p| p.as_table())
            && let Some(meta) = pkg.get("metadata").and_then(|m| m.as_table())
            && let Some(op) = meta
                .get("purist")
                .or_else(|| meta.get("opinionated"))
                .and_then(|o| o.as_table())
        {
            config.parse_metadata_purist(op);
            if let Some(lints) = op.get("lints").and_then(|l| l.as_table()) {
                config.parse_rules_table(lints);
            }
        }

        if let Some(ws) = doc.get("workspace").and_then(|w| w.as_table())
            && let Some(meta) = ws.get("metadata").and_then(|m| m.as_table())
            && let Some(op) = meta
                .get("purist")
                .or_else(|| meta.get("opinionated"))
                .and_then(|o| o.as_table())
        {
            config.parse_metadata_purist(op);
            if let Some(lints) = op.get("lints").and_then(|l| l.as_table()) {
                config.parse_rules_table(lints);
            }
        }

        config
    }

    /// Inherits configuration settings from a workspace root `LintConfig`.
    pub(crate) fn inherit_from_workspace(&mut self, ws_config: &Self) {
        self.rules = ws_config.rules.clone();
        if self.max_file_lines.max_production_lines.is_none() {
            self.max_file_lines.max_production_lines =
                ws_config.max_file_lines.max_production_lines;
        }
        if self.max_file_lines.max_total_lines.is_none() {
            self.max_file_lines.max_total_lines = ws_config.max_file_lines.max_total_lines;
        }
        if self.max_nesting_depth.max_depth.is_none() {
            self.max_nesting_depth.max_depth = ws_config.max_nesting_depth.max_depth;
        }
        if self.lib_facade_hygiene.max_production_lines.is_none() {
            self.lib_facade_hygiene.max_production_lines =
                ws_config.lib_facade_hygiene.max_production_lines;
        }
        if self.lib_facade_hygiene.max_fn_lines.is_none() {
            self.lib_facade_hygiene.max_fn_lines = ws_config.lib_facade_hygiene.max_fn_lines;
        }
    }

    pub fn from_manifest_file(file: &Path) -> Option<Self> {
        let content = fs::read_to_string(file).ok()?;
        Self::from_manifest_content(&content)
    }

    /// Parses metadata settings from `[package.metadata.purist]` or `[workspace.metadata.purist]`.
    pub(crate) fn parse_metadata_purist(&mut self, table: &toml_edit::Table) {
        self.max_file_lines
            .parse_from_metadata(table, &mut self.invalid_values);
        self.lib_facade_hygiene
            .parse_from_metadata(table, &mut self.invalid_values);
        if let Some(val) = table.get("max_nesting_depth").and_then(|v| v.as_integer()) {
            self.max_nesting_depth.max_depth = usize::try_from(val).ok();
        }
        if let Some(sub) = table.get("max_nesting_depth").and_then(|t| t.as_table())
            && let Some(val) = sub.get("max_depth").and_then(|v| v.as_integer())
        {
            self.max_nesting_depth.max_depth = usize::try_from(val).ok();
        }
    }

    /// Parses purist lint rules from a `[lints.purist]` table.
    pub(crate) fn parse_rules_table(&mut self, table: &toml_edit::Table) {
        for (key, item) in table.iter() {
            if let Some(val_str) = item.as_str() {
                if let Some(level) = RuleLevel::parse(val_str) {
                    self.set_rule(key, level);
                } else {
                    self.invalid_values.push(format!(
                        "Invalid level '{val_str}' for rule '{key}' in Cargo.toml: expected 'allow', 'warn', 'deny', or 'forbid'."
                    ));
                }
            } else if let Some(inline_table) = item.as_inline_table() {
                if let Some(level_str) = inline_table.get("level").and_then(|l| l.as_str()) {
                    if let Some(level) = RuleLevel::parse(level_str) {
                        self.set_rule(key, level);
                    } else {
                        self.invalid_values.push(format!(
                            "Invalid level '{level_str}' for rule '{key}' in Cargo.toml: expected 'allow', 'warn', 'deny', or 'forbid'."
                        ));
                    }
                }
                if key == "max_file_lines" {
                    self.max_file_lines
                        .parse_from_inline_table(inline_table, &mut self.invalid_values);
                }
                if key == "lib_facade_hygiene" {
                    self.lib_facade_hygiene
                        .parse_from_inline_table(inline_table, &mut self.invalid_values);
                }
                if key == "max_nesting_depth"
                    && let Some(val) = inline_table.get("max_depth").and_then(|v| v.as_integer())
                {
                    self.max_nesting_depth.max_depth = usize::try_from(val).ok();
                }
            } else if let Some(tbl) = item.as_table() {
                if let Some(level_str) = tbl.get("level").and_then(|l| l.as_str()) {
                    if let Some(level) = RuleLevel::parse(level_str) {
                        self.set_rule(key, level);
                    } else {
                        self.invalid_values.push(format!(
                            "Invalid level '{level_str}' for rule '{key}' in Cargo.toml: expected 'allow', 'warn', 'deny', or 'forbid'."
                        ));
                    }
                }
                if key == "max_file_lines" {
                    self.max_file_lines
                        .parse_from_rule_table(tbl, &mut self.invalid_values);
                }
                if key == "lib_facade_hygiene" {
                    self.lib_facade_hygiene
                        .parse_from_rule_table(tbl, &mut self.invalid_values);
                }
                if key == "max_nesting_depth"
                    && let Some(val) = tbl.get("max_depth").and_then(|v| v.as_integer())
                {
                    self.max_nesting_depth.max_depth = usize::try_from(val).ok();
                }
            }
        }
    }
}

/// Represents a parsed Cargo manifest (`Cargo.toml`) and its associated lint configuration.
#[derive(Debug, Clone)]
pub(crate) struct CargoManifest {
    doc: DocumentMut,
    lint_config: LintConfig,
}

impl CargoManifest {
    /// Loads and parses the `Cargo.toml` file at `manifest_or_project_dir`.
    ///
    /// If passed a directory, looks for `Cargo.toml` inside it.
    /// Inherits workspace configurations if `[lints] workspace = true`.
    pub(crate) fn load(manifest_or_project_dir: &Path) -> Result<Self, PuristError> {
        let manifest_path = if manifest_or_project_dir.is_file()
            && manifest_or_project_dir
                .file_name()
                .is_some_and(|n| n == "Cargo.toml")
        {
            manifest_or_project_dir.to_path_buf()
        } else {
            manifest_or_project_dir.join("Cargo.toml")
        };

        if !manifest_path.is_file() {
            let dir = if manifest_or_project_dir.is_file() {
                manifest_or_project_dir
                    .parent()
                    .unwrap_or(manifest_or_project_dir)
            } else {
                manifest_or_project_dir
            };
            return Err(PuristError::CargoTomlNotFound(dir.to_path_buf()));
        }

        let content = fs::read_to_string(&manifest_path)?;
        Self::from_content(manifest_path, &content)
    }

    /// Parses a manifest from its string content at a specified path.
    pub(crate) fn from_content(manifest_path: PathBuf, content: &str) -> Result<Self, PuristError> {
        let doc: DocumentMut = content
            .parse()
            .map_err(|e| PuristError::ManifestParse(format!("{e}")))?;
        let mut lint_config = LintConfig::from_document(&doc);

        // Workspace inheritance
        if has_lints_workspace_true_doc(&doc)
            && let Some(ws_path) = find_workspace_cargo_toml(&manifest_path)
            && ws_path != manifest_path
            && let Ok(ws_manifest) = CargoManifest::load(&ws_path)
        {
            lint_config.inherit_from_workspace(ws_manifest.lint_config());
        }

        Ok(Self { doc, lint_config })
    }

    pub(crate) fn document(&self) -> &DocumentMut {
        &self.doc
    }

    pub(crate) fn lint_config(&self) -> &LintConfig {
        &self.lint_config
    }

    pub(crate) fn into_lint_config(self) -> LintConfig {
        self.lint_config
    }

    pub(crate) fn has_lints_workspace_true(&self) -> bool {
        has_lints_workspace_true_doc(&self.doc)
    }
}

fn has_lints_workspace_true_doc(doc: &DocumentMut) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::default_rules;
    use googletest::prelude::*;

    #[googletest::test]
    fn parse_lint_config_from_manifest() -> Result<(), Box<dyn std::error::Error>> {
        let manifest = r#"
[package]
name = "my-crate"
version = "0.1.0"

[lints.purist]
no_wildcard_imports = "allow"
no_boxed_dyn_error = "deny"
exit_code_hygiene = { level = "warn" }
"#;
        let config = LintConfig::from_manifest_content(manifest).ok_or("valid manifest")?;
        assert_that!(
            config.level_for("no_wildcard_imports"),
            eq(Some(RuleLevel::Allow))
        );
        assert_that!(
            config.level_for("purist::no_wildcard_imports"),
            eq(Some(RuleLevel::Allow))
        );
        assert_that!(
            config.level_for("opinionated::no_wildcard_imports"),
            eq(Some(RuleLevel::Allow))
        );
        assert_that!(
            config.level_for("no_boxed_dyn_error"),
            eq(Some(RuleLevel::Deny))
        );
        assert_that!(
            config.level_for("exit_code_hygiene"),
            eq(Some(RuleLevel::Warn))
        );
        assert_that!(config.level_for("unknown_rule"), eq(None));
        Ok(())
    }

    #[googletest::test]
    fn parse_workspace_metadata_lint_config() -> Result<(), Box<dyn std::error::Error>> {
        let manifest = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.purist.lints]
centralized_command_execution = "allow"
"#;
        let config = LintConfig::from_manifest_content(manifest).ok_or("valid manifest")?;
        assert_that!(
            config.level_for("purist::centralized_command_execution"),
            eq(Some(RuleLevel::Allow))
        );
        assert_that!(
            config.level_for("opinionated::centralized_command_execution"),
            eq(Some(RuleLevel::Allow))
        );
        Ok(())
    }

    #[googletest::test]
    fn purist_lints_config_defaults_all_rules_to_warn() {
        let mut config = PuristLintsConfig::default();
        for rule in default_rules() {
            let name = rule.name();
            assert_that!(config.get(name), eq(Some(RuleLevel::Warn)));
            assert_that!(config.set(name, RuleLevel::Allow), eq(Ok(())));
            assert_that!(config.get(name), eq(Some(RuleLevel::Allow)));
        }
    }

    #[googletest::test]
    fn parse_max_nesting_depth_config() -> Result<(), Box<dyn std::error::Error>> {
        let manifest = r#"
[package.metadata.purist.max_nesting_depth]
max_depth = 3
"#;
        let config = LintConfig::from_manifest_content(manifest).ok_or("valid manifest")?;
        assert_that!(config.max_nesting_depth.max_depth, eq(Some(3)));
        Ok(())
    }

    #[googletest::test]
    fn purist_lints_config_set_updates_rule_level() {
        let mut config = PuristLintsConfig::default();
        assert_that!(
            config.set("no_wildcard_imports", RuleLevel::Allow),
            eq(Ok(()))
        );
        assert_that!(config.no_wildcard_imports, eq(RuleLevel::Allow));
        assert_that!(
            config.get("no_wildcard_imports"),
            eq(Some(RuleLevel::Allow))
        );
        assert_that!(
            config.get("purist::no_wildcard_imports"),
            eq(Some(RuleLevel::Allow))
        );
        assert_that!(
            config.get("opinionated::no_wildcard_imports"),
            eq(Some(RuleLevel::Allow))
        );
        assert_that!(
            config.set("unknown_rule", RuleLevel::Deny),
            eq(Err(UnrecognizedRule))
        );
    }

    #[googletest::test]
    fn invalid_config_values_produce_warnings() -> Result<(), Box<dyn std::error::Error>> {
        let manifest = r#"
[package]
name = "bad-config"
version = "0.1.0"

[package.metadata.purist]
max_lib_production_lines = -50
max_lib_fn_lines = "not an integer"

[lints.purist]
no_inline_mods = "invalid_level"
"#;
        let config = LintConfig::from_manifest_content(manifest).ok_or("parsed manifest")?;
        let warnings = config.warnings();
        assert_that!(
            &warnings,
            unordered_elements_are![
                contains_substring("expected a non-negative integer"),
                contains_substring("expected an integer"),
                contains_substring("expected 'allow', 'warn', 'deny', or 'forbid'")
            ]
        );
        Ok(())
    }
}
