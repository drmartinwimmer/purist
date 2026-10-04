use std::collections::HashMap;
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

/// Project-level lint configuration parsed from `Cargo.toml`.
#[derive(Debug, Clone, Default)]
pub struct LintConfig {
    rules: HashMap<String, RuleLevel>,
}

impl LintConfig {
    pub fn empty() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }

    pub fn set_rule(&mut self, rule: impl Into<String>, level: RuleLevel) {
        self.rules.insert(rule.into(), level);
    }

    pub fn level_for(&self, rule_name: &str) -> Option<RuleLevel> {
        if let Some(level) = self.rules.get(rule_name) {
            return Some(*level);
        }
        let stripped = rule_name.strip_prefix("opinionated::").unwrap_or(rule_name);
        self.rules.get(stripped).copied()
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
