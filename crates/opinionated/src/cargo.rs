use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::DocumentMut;

/// Severity configuration for an opinionated lint rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleLevel {
    Allow,
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

/// Strongly-typed struct holding configuration levels for all known opinionated lint rules.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpinionatedLintsConfig {
    pub no_inline_mods: Option<RuleLevel>,
    pub free_functions: Option<RuleLevel>,
    pub path_resolution: Option<RuleLevel>,
    pub error_types: Option<RuleLevel>,
    pub clippy_suppression_hygiene: Option<RuleLevel>,
    pub test_patterns: Option<RuleLevel>,
    pub no_redundant_conversions: Option<RuleLevel>,
    pub use_declarations_over_qualified_paths: Option<RuleLevel>,
    pub no_redundant_wrappers: Option<RuleLevel>,
    pub no_boxed_dyn_error: Option<RuleLevel>,
    pub test_matcher_borrow_simplification: Option<RuleLevel>,
    pub no_test_prefix: Option<RuleLevel>,
    pub no_unsafe_in_tests: Option<RuleLevel>,
    pub centralized_command_execution: Option<RuleLevel>,
    pub clap_struct_encapsulation: Option<RuleLevel>,
    pub exit_code_hygiene: Option<RuleLevel>,
    pub idiomatic_option_bool_mapping: Option<RuleLevel>,
    pub no_wildcard_imports: Option<RuleLevel>,
    pub no_env_access_outside_config: Option<RuleLevel>,
    pub single_match_to_let_else: Option<RuleLevel>,
    pub raii_temp_directories: Option<RuleLevel>,
    pub no_println_in_libraries: Option<RuleLevel>,
    pub cli_run_consumes_self: Option<RuleLevel>,
}

impl OpinionatedLintsConfig {
    pub fn get(&self, rule_name: &str) -> Option<RuleLevel> {
        let stripped = rule_name.strip_prefix("opinionated::").unwrap_or(rule_name);
        match stripped {
            "no_inline_mods" => self.no_inline_mods,
            "free_functions" => self.free_functions,
            "path_resolution" => self.path_resolution,
            "error_types" => self.error_types,
            "clippy_suppression_hygiene" => self.clippy_suppression_hygiene,
            "test_patterns" => self.test_patterns,
            "no_redundant_conversions" => self.no_redundant_conversions,
            "use_declarations_over_qualified_paths" => self.use_declarations_over_qualified_paths,
            "no_redundant_wrappers" => self.no_redundant_wrappers,
            "no_boxed_dyn_error" => self.no_boxed_dyn_error,
            "test_matcher_borrow_simplification" => self.test_matcher_borrow_simplification,
            "no_test_prefix" => self.no_test_prefix,
            "no_unsafe_in_tests" => self.no_unsafe_in_tests,
            "centralized_command_execution" => self.centralized_command_execution,
            "clap_struct_encapsulation" => self.clap_struct_encapsulation,
            "exit_code_hygiene" => self.exit_code_hygiene,
            "idiomatic_option_bool_mapping" => self.idiomatic_option_bool_mapping,
            "no_wildcard_imports" => self.no_wildcard_imports,
            "no_env_access_outside_config" => self.no_env_access_outside_config,
            "single_match_to_let_else" => self.single_match_to_let_else,
            "raii_temp_directories" => self.raii_temp_directories,
            "no_println_in_libraries" => self.no_println_in_libraries,
            "cli_run_consumes_self" => self.cli_run_consumes_self,
            _ => None,
        }
    }

    pub fn set(&mut self, rule_name: &str, level: RuleLevel) -> Result<(), UnrecognizedRule> {
        let stripped = rule_name.strip_prefix("opinionated::").unwrap_or(rule_name);
        match stripped {
            "no_inline_mods" => self.no_inline_mods = Some(level),
            "free_functions" => self.free_functions = Some(level),
            "path_resolution" => self.path_resolution = Some(level),
            "error_types" => self.error_types = Some(level),
            "clippy_suppression_hygiene" => self.clippy_suppression_hygiene = Some(level),
            "test_patterns" => self.test_patterns = Some(level),
            "no_redundant_conversions" => self.no_redundant_conversions = Some(level),
            "use_declarations_over_qualified_paths" => {
                self.use_declarations_over_qualified_paths = Some(level)
            }
            "no_redundant_wrappers" => self.no_redundant_wrappers = Some(level),
            "no_boxed_dyn_error" => self.no_boxed_dyn_error = Some(level),
            "test_matcher_borrow_simplification" => {
                self.test_matcher_borrow_simplification = Some(level)
            }
            "no_test_prefix" => self.no_test_prefix = Some(level),
            "no_unsafe_in_tests" => self.no_unsafe_in_tests = Some(level),
            "centralized_command_execution" => self.centralized_command_execution = Some(level),
            "clap_struct_encapsulation" => self.clap_struct_encapsulation = Some(level),
            "exit_code_hygiene" => self.exit_code_hygiene = Some(level),
            "idiomatic_option_bool_mapping" => self.idiomatic_option_bool_mapping = Some(level),
            "no_wildcard_imports" => self.no_wildcard_imports = Some(level),
            "no_env_access_outside_config" => self.no_env_access_outside_config = Some(level),
            "single_match_to_let_else" => self.single_match_to_let_else = Some(level),
            "raii_temp_directories" => self.raii_temp_directories = Some(level),
            "no_println_in_libraries" => self.no_println_in_libraries = Some(level),
            "cli_run_consumes_self" => self.cli_run_consumes_self = Some(level),
            _ => return Err(UnrecognizedRule),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnrecognizedRule;

/// Map of deprecated rule aliases to their canonical rule names.
const DEPRECATED_ALIASES: &[(&str, &str)] = &[
    ("clippy_suppress", "clippy_suppression_hygiene"),
    ("use_declarations", "use_declarations_over_qualified_paths"),
    ("centralized_commands", "centralized_command_execution"),
    ("clap_encapsulation", "clap_struct_encapsulation"),
    ("option_bool_mapping", "idiomatic_option_bool_mapping"),
    ("test_matcher_borrow", "test_matcher_borrow_simplification"),
];

/// Project-level lint configuration parsed from `Cargo.toml`.
#[derive(Debug, Clone, Default)]
pub struct LintConfig {
    pub rules: OpinionatedLintsConfig,
    pub unrecognized: Vec<String>,
    pub deprecated: Vec<String>,
}

impl LintConfig {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn set_rule(&mut self, rule: impl Into<String>, level: RuleLevel) {
        let name = rule.into();
        let stripped = name.strip_prefix("opinionated::").unwrap_or(&name);

        if let Some((_, canonical)) = DEPRECATED_ALIASES
            .iter()
            .find(|(alias, _)| *alias == stripped)
        {
            self.deprecated.push(format!(
                "Rule 'opinionated::{stripped}' is deprecated. Use 'opinionated::{canonical}' instead."
            ));
            if let Err(UnrecognizedRule) = self.rules.set(canonical, level) {
                self.unrecognized.push(canonical.to_string());
            }
        } else if self.rules.set(stripped, level).is_err() {
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
                "Unrecognized opinionated lint rule '{unrec}' specified in Cargo.toml."
            ));
        }
        warnings
    }

    /// Loads lint configuration from the nearest `Cargo.toml` at or above `path`.
    pub fn discover_for_path(path: &Path) -> Self {
        if let Some(cargo_toml) = find_cargo_toml(path) {
            Self::from_manifest_file(&cargo_toml).unwrap_or_default()
        } else {
            Self::empty()
        }
    }

    /// Parses lint configuration from a `Cargo.toml` file content.
    pub fn from_manifest_content(content: &str) -> Option<Self> {
        let doc: DocumentMut = content.parse().ok()?;
        let mut config = Self::empty();

        // 1. Check [lints.opinionated] or [workspace.lints.opinionated]
        if let Some(lints) = doc.get("lints").and_then(|l| l.as_table())
            && let Some(opinionated) = lints.get("opinionated").and_then(|o| o.as_table())
        {
            parse_rules_table(opinionated, &mut config);
        }

        if let Some(ws) = doc.get("workspace").and_then(|w| w.as_table())
            && let Some(lints) = ws.get("lints").and_then(|l| l.as_table())
            && let Some(opinionated) = lints.get("opinionated").and_then(|o| o.as_table())
        {
            parse_rules_table(opinionated, &mut config);
        }

        // 2. Check [package.metadata.opinionated.lints] or [workspace.metadata.opinionated.lints]
        if let Some(pkg) = doc.get("package").and_then(|p| p.as_table())
            && let Some(meta) = pkg.get("metadata").and_then(|m| m.as_table())
            && let Some(op) = meta.get("opinionated").and_then(|o| o.as_table())
            && let Some(lints) = op.get("lints").and_then(|l| l.as_table())
        {
            parse_rules_table(lints, &mut config);
        }

        if let Some(ws) = doc.get("workspace").and_then(|w| w.as_table())
            && let Some(meta) = ws.get("metadata").and_then(|m| m.as_table())
            && let Some(op) = meta.get("opinionated").and_then(|o| o.as_table())
            && let Some(lints) = op.get("lints").and_then(|l| l.as_table())
        {
            parse_rules_table(lints, &mut config);
        }

        Some(config)
    }

    pub fn from_manifest_file(file: &Path) -> Option<Self> {
        let content = fs::read_to_string(file).ok()?;
        Self::from_manifest_content(&content)
    }
}

fn parse_rules_table(table: &toml_edit::Table, config: &mut LintConfig) {
    for (key, item) in table.iter() {
        if let Some(val_str) = item.as_str() {
            if let Some(level) = RuleLevel::parse(val_str) {
                config.set_rule(key, level);
            }
        } else if let Some(inline_table) = item.as_inline_table() {
            if let Some(level_str) = inline_table.get("level").and_then(|l| l.as_str())
                && let Some(level) = RuleLevel::parse(level_str)
            {
                config.set_rule(key, level);
            }
        } else if let Some(tbl) = item.as_table()
            && let Some(level_str) = tbl.get("level").and_then(|l| l.as_str())
            && let Some(level) = RuleLevel::parse(level_str)
        {
            config.set_rule(key, level);
        }
    }
}

/// Discovers Rust files for analysis similar to Cargo/Clippy, guided by `Cargo.toml`.
pub fn discover_rust_files(target: &Path) -> Vec<PathBuf> {
    if target.is_file() {
        return vec![target.to_path_buf()];
    }

    // Try finding Cargo.toml at the target directory
    let manifest_path = if target.join("Cargo.toml").is_file() {
        Some(target.join("Cargo.toml"))
    } else {
        find_cargo_toml(target)
    };

    if let Some(cargo_file) = manifest_path
        && let Some(parent_dir) = cargo_file.parent()
        && let Ok(content) = fs::read_to_string(&cargo_file)
        && let Ok(doc) = content.parse::<DocumentMut>()
    {
        let mut collected = Vec::new();

        // If virtual workspace with members
        if let Some(ws) = doc.get("workspace").and_then(|w| w.as_table())
            && let Some(members) = ws.get("members").and_then(|m| m.as_array())
        {
            for member in members {
                if let Some(member_str) = member.as_str() {
                    collect_member_targets(parent_dir, member_str, &mut collected);
                }
            }
        }

        // Also check root package targets
        if doc.contains_key("package") {
            collect_package_targets(parent_dir, &doc, &mut collected);
        }

        if !collected.is_empty() {
            collected.sort();
            collected.dedup();
            return collected;
        }
    }

    // Fallback: directory crawl skipping build artifacts and hidden directories
    let mut files = Vec::new();
    crawl_directory_fallback(target, &mut files);
    files.sort();
    files.dedup();
    files
}

fn collect_member_targets(root: &Path, member_pattern: &str, files: &mut Vec<PathBuf>) {
    let clean_pattern = member_pattern
        .trim_end_matches("/*")
        .trim_end_matches("/**");
    let base_path = root.join(clean_pattern);

    if member_pattern.contains('*') {
        if let Ok(entries) = fs::read_dir(&base_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("Cargo.toml").is_file() {
                    if let Ok(content) = fs::read_to_string(path.join("Cargo.toml"))
                        && let Ok(doc) = content.parse::<DocumentMut>()
                    {
                        collect_package_targets(&path, &doc, files);
                    } else {
                        collect_standard_package_dirs(&path, files);
                    }
                }
            }
        }
    } else if base_path.is_dir() {
        if let Ok(content) = fs::read_to_string(base_path.join("Cargo.toml"))
            && let Ok(doc) = content.parse::<DocumentMut>()
        {
            collect_package_targets(&base_path, &doc, files);
        } else {
            collect_standard_package_dirs(&base_path, files);
        }
    }
}

fn collect_package_targets(package_dir: &Path, doc: &DocumentMut, files: &mut Vec<PathBuf>) {
    // 1. Explicit [lib] path
    if let Some(lib) = doc.get("lib").and_then(|l| l.as_table())
        && let Some(path_val) = lib.get("path").and_then(|p| p.as_str())
    {
        let p = package_dir.join(path_val);
        if p.is_file() {
            files.push(p);
        }
    }

    // 2. Explicit [[bin]] paths
    if let Some(bins) = doc.get("bin").and_then(|b| b.as_array_of_tables()) {
        for bin in bins {
            if let Some(path_val) = bin.get("path").and_then(|p| p.as_str()) {
                let p = package_dir.join(path_val);
                if p.is_file() {
                    files.push(p);
                }
            }
        }
    }

    // 3. Scan standard cargo target folders: src/, tests/, examples/, benches/
    collect_standard_package_dirs(package_dir, files);
}

fn collect_standard_package_dirs(package_dir: &Path, files: &mut Vec<PathBuf>) {
    for folder in &["src", "tests", "examples", "benches"] {
        let dir = package_dir.join(folder);
        if dir.is_dir() {
            crawl_directory_fallback(&dir, files);
        }
    }
}

fn crawl_directory_fallback(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str())
                && (name.starts_with('.') || name == "target")
            {
                continue;
            }
            if path.is_dir() {
                crawl_directory_fallback(&path, files);
            } else if path.is_file() && path.extension().map(|e| e == "rs").unwrap_or(false) {
                files.push(path);
            }
        }
    }
}

pub fn find_cargo_toml(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let candidate = current.join("Cargo.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            break;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn parse_lint_config_from_manifest() {
        let manifest = r#"
[package]
name = "my-crate"
version = "0.1.0"

[lints.opinionated]
no_wildcard_imports = "allow"
no_boxed_dyn_error = "deny"
exit_code_hygiene = { level = "warn" }
"#;
        let config = LintConfig::from_manifest_content(manifest).expect("valid manifest");
        assert_that!(
            config.level_for("no_wildcard_imports"),
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
    }

    #[googletest::test]
    fn parse_workspace_metadata_lint_config() {
        let manifest = r#"
[workspace]
members = ["crates/*"]

[workspace.metadata.opinionated.lints]
centralized_command_execution = "allow"
"#;
        let config = LintConfig::from_manifest_content(manifest).expect("valid manifest");
        assert_that!(
            config.level_for("opinionated::centralized_command_execution"),
            eq(Some(RuleLevel::Allow))
        );
    }
}
