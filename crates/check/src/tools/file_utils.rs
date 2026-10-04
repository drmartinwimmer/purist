use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Checks if an external command is installed and executable in PATH.
pub fn is_tool_available(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Recursively discovers files with matching extensions, ignoring hidden directories and build artifacts.
pub fn find_files_with_extensions(root: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    if root.is_file() {
        if let Some(ext) = root.extension().and_then(|s| s.to_str())
            && extensions.iter().any(|e| ext.eq_ignore_ascii_case(e))
        {
            return vec![root.to_path_buf()];
        }
        return Vec::new();
    }

    let mut results = Vec::new();
    crawl_directory(root, extensions, &mut results);
    results.sort();
    results
}

fn crawl_directory(dir: &Path, extensions: &[&str], results: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };

        if path.is_dir() {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            crawl_directory(&path, extensions, results);
        } else if path.is_file()
            && let Some(ext) = path.extension().and_then(|s| s.to_str())
            && extensions.iter().any(|e| ext.eq_ignore_ascii_case(e))
        {
            results.push(path);
        }
    }
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
    fn find_files_with_extensions_locates_files_and_skips_hidden_dirs()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir().join(format!("test_find_files_{}", std::process::id()));
        fs::create_dir_all(temp_dir.join("subdir"))?;
        fs::create_dir_all(temp_dir.join(".hidden"))?;
        fs::create_dir_all(temp_dir.join("target"))?;
        let _guard = TempDirGuard(temp_dir.clone());

        fs::write(temp_dir.join("doc.md"), "# Title")?;
        fs::write(temp_dir.join("subdir/nested.md"), "# Nested")?;
        fs::write(temp_dir.join(".hidden/secret.md"), "# Secret")?;
        fs::write(temp_dir.join("target/ignored.md"), "# Ignored")?;
        fs::write(temp_dir.join("config.json"), "{}")?;

        let md_files = find_files_with_extensions(&temp_dir, &["md"]);
        expect_that!(md_files.len(), eq(2));
        expect_that!(md_files.contains(&temp_dir.join("doc.md")), is_true());
        expect_that!(
            md_files.contains(&temp_dir.join("subdir/nested.md")),
            is_true()
        );
        expect_that!(
            md_files.contains(&temp_dir.join(".hidden/secret.md")),
            is_false()
        );
        expect_that!(
            md_files.contains(&temp_dir.join("target/ignored.md")),
            is_false()
        );
        Ok(())
    }

    #[googletest::test]
    fn is_tool_available_returns_false_for_nonexistent_binary() {
        assert_that!(
            is_tool_available("definitely_nonexistent_binary_xyz123"),
            is_false()
        );
    }
}
