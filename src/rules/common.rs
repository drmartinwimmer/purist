//! # Shared AST and Lint Inspection Helpers
//!
//! Provides reusable AST inspection utilities for purist lint rules, including:
//! - Attribute detection (`#[cfg(test)]`, `#[test]`, derive traits, suppressions)
//! - Test scope tracking across modules and functions (`TestScopeTracker`)
//! - Path and macro segment inspection helpers (`path_ends_with_segments`, `macro_name`, `extract_type_ident`)
//! - Trait implementation detection (e.g. `Drop`)

use syn::{Attribute, Ident, ItemImpl, Macro, Path, Type};

/// Checks whether an attribute matches `#[cfg(test)]`.
pub fn is_cfg_test_attr(attr: &Attribute) -> bool {
    if !attr.path().is_ident("cfg") {
        return false;
    }
    let mut is_test = false;
    let _result = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("test") {
            is_test = true;
        }
        Ok(())
    });
    is_test
}

/// Checks whether an attribute list includes `#[cfg(test)]`.
pub fn has_cfg_test_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_cfg_test_attr)
}

/// Checks whether an attribute matches bare `#[test]`.
pub fn is_bare_test_attr(attr: &Attribute) -> bool {
    attr.path().is_ident("test")
}

/// Checks whether an attribute list contains bare `#[test]`.
pub fn has_bare_test_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_bare_test_attr)
}

/// Checks whether an attribute matches framework test macros such as `#[googletest::test]`, `#[rstest]`, etc.
pub fn is_framework_test_attr(attr: &Attribute) -> bool {
    if attr.path().is_ident("rstest") {
        return true;
    }
    let segs = &attr.path().segments;
    segs.len() >= 2 && segs.last().is_some_and(|s| s.ident == "test")
}

/// Checks whether an attribute list contains any framework test attribute (`#[googletest::test]`, `#[rstest]`, etc.).
pub fn has_framework_test_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_framework_test_attr)
}

/// Checks whether an attribute matches `#[test]` or any framework test attribute ending in `::test`.
pub fn is_test_attr(attr: &Attribute) -> bool {
    is_bare_test_attr(attr) || is_framework_test_attr(attr)
}

/// Checks whether an attribute list contains `#[test]` or any framework test attribute.
pub fn has_test_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_test_attr)
}

/// Checks whether an attribute list derives any of the specified trait names.
pub fn derives_any(attrs: &[Attribute], traits: &[&str]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let mut found = false;
        let _result = attr.parse_nested_meta(|meta| {
            for &trait_name in traits {
                if meta.path.is_ident(trait_name)
                    || meta
                        .path
                        .segments
                        .last()
                        .is_some_and(|s| s.ident == trait_name)
                {
                    found = true;
                    break;
                }
            }
            Ok(())
        });
        found
    })
}

/// Checks whether an attribute list derives the specified trait name.
pub fn derives_trait(attrs: &[Attribute], trait_name: &str) -> bool {
    derives_any(attrs, &[trait_name])
}

/// Checks whether an attribute list contains an `#[allow(...)]` or `#[expect(...)]` suppression for `rule_name`.
pub fn has_suppression_attribute(attrs: &[Attribute], rule_name: &str) -> bool {
    let short_name = rule_name.strip_prefix("purist::").unwrap_or(rule_name);
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("expect") && !attr.path().is_ident("allow") {
            return false;
        }
        let mut matched = false;
        let _result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(short_name)
                || meta.path.is_ident(rule_name)
                || meta
                    .path
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == short_name)
            {
                matched = true;
            }
            Ok(())
        });
        matched
    })
}

/// Tracks lexical test scopes (test files, `#[cfg(test)]` modules, and test functions) during AST traversal.
#[derive(Debug, Clone, Copy)]
pub struct TestScopeTracker {
    in_test_module: bool,
    in_test_fn: bool,
}

impl TestScopeTracker {
    /// Creates a new test scope tracker, initialized with whether the current file is a test file.
    pub fn new(is_test_file: bool) -> Self {
        Self {
            in_test_module: is_test_file,
            in_test_fn: false,
        }
    }

    /// Returns true if currently within any test context (test file, `#[cfg(test)]` module, or test function).
    pub fn is_in_test(&self) -> bool {
        self.in_test_module || self.in_test_fn
    }

    /// Returns true if currently within a `#[cfg(test)]` module or a dedicated test file.
    pub fn is_in_test_module(&self) -> bool {
        self.in_test_module
    }

    /// Returns true if currently within the body of a test function.
    pub fn is_in_test_fn(&self) -> bool {
        self.in_test_fn
    }

    /// Updates the tracker when entering a module with attributes, returning the previous module state.
    pub fn enter_mod(&mut self, attrs: &[Attribute]) -> bool {
        let prev = self.in_test_module;
        if has_cfg_test_attr(attrs) {
            self.in_test_module = true;
        }
        prev
    }

    /// Restores the previous module test state when exiting a module.
    pub fn exit_mod(&mut self, prev: bool) {
        self.in_test_module = prev;
    }

    /// Updates the tracker when entering a function, returning the previous function state.
    pub fn enter_fn(&mut self, attrs: &[Attribute]) -> bool {
        let prev = self.in_test_fn;
        if has_test_attr(attrs) {
            self.in_test_fn = true;
        }
        prev
    }

    /// Updates the tracker when entering a function, also treating functions in test modules
    /// starting with `test_` as test functions. Returns the previous function state.
    pub fn enter_fn_with_name(&mut self, attrs: &[Attribute], fn_name: &str) -> bool {
        let prev = self.in_test_fn;
        if has_test_attr(attrs) || (self.in_test_module && fn_name.starts_with("test_")) {
            self.in_test_fn = true;
        }
        prev
    }

    /// Restores the previous function test state when exiting a function.
    pub fn exit_fn(&mut self, prev: bool) {
        self.in_test_fn = prev;
    }
}

/// Returns the identifier of the last segment in a path.
pub fn path_last_ident(path: &Path) -> Option<&Ident> {
    path.segments.last().map(|s| &s.ident)
}

/// Checks whether the final segment of a path matches `expected`.
pub fn path_ends_with_ident(path: &Path, expected: &str) -> bool {
    path.segments.last().is_some_and(|s| s.ident == expected)
}

/// Checks whether the suffix of path segments matches the given sequence of segment identifiers.
pub fn path_ends_with_segments(path: &Path, expected: &[&str]) -> bool {
    if path.segments.len() < expected.len() {
        return false;
    }
    let offset = path.segments.len() - expected.len();
    path.segments
        .iter()
        .skip(offset)
        .zip(expected.iter())
        .all(|(seg, &expected_seg)| seg.ident == expected_seg)
}

/// Returns the macro name identifier from its invocation path.
pub fn macro_name(mac: &Macro) -> Option<&Ident> {
    path_last_ident(&mac.path)
}

/// Extracts the type identifier if the given type is a simple unqualified path.
pub fn extract_type_ident(ty: &Type) -> Option<&Ident> {
    if let Type::Path(type_path) = ty
        && type_path.qself.is_none()
    {
        return path_last_ident(&type_path.path);
    }
    None
}

/// Checks whether an implementation block implements the `Drop` trait.
pub fn is_drop_trait_impl(item_impl: &ItemImpl) -> bool {
    item_impl
        .trait_
        .as_ref()
        .is_some_and(|(_, path, _)| path_ends_with_ident(path, "Drop"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn cfg_test_detection_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let item_mod: syn::ItemMod = syn::parse_str("#[cfg(test)] mod tests {}")?;
        assert_that!(has_cfg_test_attr(&item_mod.attrs), eq(true));

        let non_test_mod: syn::ItemMod = syn::parse_str("#[cfg(feature = \"foo\")] mod foo {}")?;
        assert_that!(has_cfg_test_attr(&non_test_mod.attrs), eq(false));
        Ok(())
    }

    #[googletest::test]
    fn detect_test_attributes_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let bare_fn: syn::ItemFn = syn::parse_str("#[test] fn my_test() {}")?;
        assert_that!(has_bare_test_attr(&bare_fn.attrs), eq(true));
        assert_that!(has_test_attr(&bare_fn.attrs), eq(true));

        let framework_fn: syn::ItemFn = syn::parse_str("#[googletest::test] fn my_test() {}")?;
        assert_that!(has_bare_test_attr(&framework_fn.attrs), eq(false));
        assert_that!(has_framework_test_attr(&framework_fn.attrs), eq(true));
        assert_that!(has_test_attr(&framework_fn.attrs), eq(true));

        let rstest_fn: syn::ItemFn = syn::parse_str("#[rstest] fn my_test() {}")?;
        assert_that!(has_framework_test_attr(&rstest_fn.attrs), eq(true));
        assert_that!(has_test_attr(&rstest_fn.attrs), eq(true));

        let normal_fn: syn::ItemFn = syn::parse_str("fn normal() {}")?;
        assert_that!(has_test_attr(&normal_fn.attrs), eq(false));
        Ok(())
    }

    #[googletest::test]
    fn derives_any_detection_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let item_struct: syn::ItemStruct =
            syn::parse_str("#[derive(clap::Parser, Debug)] struct Cli;")?;
        assert_that!(
            derives_any(&item_struct.attrs, &["Parser", "Args"]),
            eq(true)
        );
        assert_that!(derives_trait(&item_struct.attrs, "Parser"), eq(true));
        assert_that!(derives_trait(&item_struct.attrs, "Clone"), eq(false));
        Ok(())
    }

    #[googletest::test]
    fn track_test_scope_transitions_correctly() -> Result<(), Box<dyn std::error::Error>> {
        let mut tracker = TestScopeTracker::new(false);
        assert_that!(tracker.is_in_test(), eq(false));

        let test_mod: syn::ItemMod = syn::parse_str("#[cfg(test)] mod tests {}")?;
        let prev_mod = tracker.enter_mod(&test_mod.attrs);
        assert_that!(tracker.is_in_test(), eq(true));
        assert_that!(tracker.is_in_test_module(), eq(true));

        tracker.exit_mod(prev_mod);
        assert_that!(tracker.is_in_test(), eq(false));

        let test_fn: syn::ItemFn = syn::parse_str("#[test] fn check() {}")?;
        let prev_fn = tracker.enter_fn(&test_fn.attrs);
        assert_that!(tracker.is_in_test(), eq(true));
        assert_that!(tracker.is_in_test_fn(), eq(true));

        tracker.exit_fn(prev_fn);
        assert_that!(tracker.is_in_test(), eq(false));
        Ok(())
    }

    #[googletest::test]
    fn path_matching_helpers_work() -> Result<(), Box<dyn std::error::Error>> {
        let path: syn::Path = syn::parse_str("std::process::Command::new")?;
        assert_that!(path_ends_with_ident(&path, "new"), eq(true));
        assert_that!(
            path_ends_with_segments(&path, &["Command", "new"]),
            eq(true)
        );
        assert_that!(
            path_ends_with_segments(&path, &["process", "Command", "new"]),
            eq(true)
        );
        assert_that!(path_ends_with_segments(&path, &["Command"]), eq(false));

        let ty: syn::Type = syn::parse_str("MyStruct")?;
        let ident = extract_type_ident(&ty).ok_or("expected ident")?;
        assert_that!(&ident.to_string(), eq("MyStruct"));

        let drop_impl: syn::ItemImpl =
            syn::parse_str("impl Drop for MyStruct { fn drop(&mut self) {} }")?;
        assert_that!(is_drop_trait_impl(&drop_impl), eq(true));
        Ok(())
    }
}
