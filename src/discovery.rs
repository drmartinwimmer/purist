use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::DocumentMut;

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
        let Ok(entries) = fs::read_dir(&base_path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("Cargo.toml").is_file() {
                collect_package_or_standard_dirs(&path, files);
            }
        }
    } else if base_path.is_dir() {
        collect_package_or_standard_dirs(&base_path, files);
    }
}

fn collect_package_or_standard_dirs(path: &Path, files: &mut Vec<PathBuf>) {
    let manifest_path = path.join("Cargo.toml");
    if let Ok(content) = fs::read_to_string(&manifest_path)
        && let Ok(doc) = content.parse::<DocumentMut>()
    {
        collect_package_targets(path, &doc, files);
    } else {
        collect_standard_package_dirs(path, files);
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
            } else if path.is_file() && path.extension().is_some_and(|e| e == "rs") {
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

/// Finds the enclosing workspace `Cargo.toml` if `start` is located within a workspace member.
pub fn find_workspace_cargo_toml(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    while current.pop() {
        let candidate = current.join("Cargo.toml");
        if candidate.is_file()
            && let Ok(content) = fs::read_to_string(&candidate)
            && content.contains("[workspace]")
        {
            return Some(candidate);
        }
    }

    None
}
