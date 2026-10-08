use crate::PuristError;
use crate::cargo::CargoManifest;
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::DocumentMut;

/// Discovers all Rust files for a Cargo project from its pre-loaded `manifest`.
///
/// Discovery is strictly focused on targets defined by the manifest,
/// including all workspace members and child crates, with no directory-crawling fallbacks.
pub fn discover_project_files_from_manifest(
    project_dir: &Path,
    manifest: &CargoManifest,
) -> Result<Vec<PathBuf>, PuristError> {
    let doc = manifest.document();
    let mut collected = Vec::new();
    let mut package_dirs = Vec::new();

    // 1. Root package (if defined)
    if doc.contains_key("package") {
        package_dirs.push(project_dir.to_path_buf());
    }

    // 2. Workspace members (if defined)
    if let Some(ws) = doc.get("workspace").and_then(|w| w.as_table())
        && let Some(members) = ws.get("members").and_then(|m| m.as_array())
    {
        for member in members {
            if let Some(member_str) = member.as_str() {
                collect_member_package_dirs(project_dir, member_str, &mut package_dirs);
            }
        }
    }

    // 3. Collect child crates within the project directory (subdirectories with Cargo.toml)
    collect_child_package_dirs(project_dir, &mut package_dirs);

    package_dirs.sort();
    package_dirs.dedup();

    // Collect targets from each package directory
    for pkg_dir in &package_dirs {
        if pkg_dir == project_dir {
            collect_package_targets(pkg_dir, doc, &mut collected);
        } else {
            collect_package_or_standard_dirs(pkg_dir, &mut collected);
        }
    }

    // Ensure all discovered files are strictly within the project directory
    let canonical_root = project_dir.canonicalize().ok();
    collected.retain(|path| {
        if let (Some(root), Ok(canon)) = (&canonical_root, path.canonicalize()) {
            canon.starts_with(root)
        } else {
            path.starts_with(project_dir)
        }
    });

    collected.sort();
    collected.dedup();
    Ok(collected)
}

/// Discovers all Rust files for a Cargo project rooted at `project_dir`.
///
/// Requires a `Cargo.toml` to be present in `project_dir`.
/// Discovery is strictly focused on targets defined by the manifest,
/// including all workspace members and child crates, with no directory-crawling fallbacks.
pub fn discover_project_files(project_dir: &Path) -> Result<Vec<PathBuf>, PuristError> {
    let manifest = CargoManifest::load(project_dir)?;
    discover_project_files_from_manifest(project_dir, &manifest)
}

/// Backwards-compatible helper that discovers Rust files for a target path.
///
/// If `target` is a file, returns a singleton vector containing it.
/// If `target` is a directory with a `Cargo.toml`, discovers all files in that project.
/// Returns an empty vector if no `Cargo.toml` is found (no fallbacks).
pub fn discover_rust_files(target: &Path) -> Vec<PathBuf> {
    if target.is_file() {
        return vec![target.to_path_buf()];
    }

    discover_project_files(target).unwrap_or_default()
}

fn collect_member_package_dirs(root: &Path, member_pattern: &str, dirs: &mut Vec<PathBuf>) {
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
                dirs.push(path);
            }
        }
    } else if base_path.is_dir() && base_path.join("Cargo.toml").is_file() {
        dirs.push(base_path);
    }
}

fn collect_child_package_dirs(root: &Path, dirs: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|s| s.to_str())
            && (name.starts_with('.') || name == "target")
        {
            continue;
        }
        if path.is_dir() {
            if path.join("Cargo.toml").is_file() {
                dirs.push(path);
            } else {
                collect_child_package_dirs(&path, dirs);
            }
        }
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
            crawl_target_folder(&dir, files);
        }
    }
}

fn crawl_target_folder(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str())
                && (name.starts_with('.') || name == "target")
            {
                continue;
            }
            if path.is_dir() {
                crawl_target_folder(&path, files);
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

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    struct TempDirGuard(PathBuf);

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.0));
        }
    }

    #[googletest::test]
    fn discover_project_files_without_cargo_toml_fails() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_disc_no_manifest_{}", std::process::id()));
        fs::create_dir_all(&temp_dir)?;
        let _guard = TempDirGuard(temp_dir.clone());

        let result = discover_project_files(&temp_dir);
        match result {
            Err(PuristError::CargoTomlNotFound(p)) => {
                assert_that!(p, eq(&temp_dir));
                Ok(())
            }
            other => Err(format!("Expected CargoTomlNotFound, got {other:?}").into()),
        }
    }

    #[googletest::test]
    fn discover_project_files_in_single_package_collects_targets()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir =
            std::env::temp_dir().join(format!("test_disc_single_pkg_{}", std::process::id()));
        fs::create_dir_all(temp_dir.join("src"))?;
        fs::create_dir_all(temp_dir.join("tests"))?;
        fs::create_dir_all(temp_dir.join("unrelated_folder"))?;
        let _guard = TempDirGuard(temp_dir.clone());

        fs::write(
            temp_dir.join("Cargo.toml"),
            "[package]\nname = \"single\"\nversion = \"0.1.0\"\n",
        )?;
        fs::write(temp_dir.join("src/lib.rs"), "pub fn foo() {}\n")?;
        fs::write(temp_dir.join("tests/test.rs"), "fn test_foo() {}\n")?;
        // This unrelated .rs file should NOT be discovered because there are no fallbacks!
        fs::write(
            temp_dir.join("unrelated_folder/ignored.rs"),
            "fn bar() {}\n",
        )?;

        let files = discover_project_files(&temp_dir)?;
        assert_that!(files.len(), eq(2));
        assert_that!(files.contains(&temp_dir.join("src/lib.rs")), is_true());
        assert_that!(files.contains(&temp_dir.join("tests/test.rs")), is_true());
        assert_that!(
            files.contains(&temp_dir.join("unrelated_folder/ignored.rs")),
            is_false()
        );
        Ok(())
    }

    #[googletest::test]
    fn discover_project_files_in_workspace_collects_member_targets()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_disc_ws_{}", std::process::id()));
        let member_dir = temp_dir.join("crates/sub");
        fs::create_dir_all(member_dir.join("src"))?;
        let _guard = TempDirGuard(temp_dir.clone());

        fs::write(
            temp_dir.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )?;
        fs::write(
            member_dir.join("Cargo.toml"),
            "[package]\nname = \"sub\"\nversion = \"0.1.0\"\n",
        )?;
        fs::write(member_dir.join("src/main.rs"), "fn main() {}\n")?;

        let files = discover_project_files(&temp_dir)?;
        assert_that!(files.len(), eq(1));
        assert_that!(files.contains(&member_dir.join("src/main.rs")), is_true());
        Ok(())
    }
}
