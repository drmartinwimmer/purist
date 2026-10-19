//! Parameterized checkers for identifier names and prefixes.

/// Determines if a test function name begins with a forbidden `test_` or `test` prefix.
pub(crate) fn check_ident_has_test_prefix(name: &str) -> bool {
    name.starts_with("test_")
        || name == "test"
        || (name.starts_with("test")
            && name
                .chars()
                .nth(4)
                .map(|c| c.is_ascii_uppercase() || c == '_')
                .unwrap_or(false))
}
