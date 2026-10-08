//! Parameterized checkers for identifier names and prefixes.

use syn::Ident;

/// Checks whether an identifier starts with any of the specified prefixes.
pub fn check_ident_has_prefix<'a>(ident: &Ident, prefixes: &[&'a str]) -> Option<&'a str> {
    let name = ident.to_string();
    prefixes
        .iter()
        .copied()
        .find(|prefix| name.starts_with(prefix))
}

/// Checks whether an identifier matches negative boolean naming conventions.
///
/// Returns true if the identifier begins with negative prefixes (e.g. `skip_`, `no_`, `not_`,
/// `without_`, `disable_`, `disabled_`, `disallow_`) or matches exact negative words.
pub fn check_ident_has_negative_name(name: &str) -> bool {
    const NEGATIVE_PREFIXES: &[&str] = &[
        "skip_",
        "no_",
        "not_",
        "without_",
        "disable_",
        "disabled_",
        "disallow_",
    ];
    const NEGATIVE_EXACT_WORDS: &[&str] = &["skip", "disable", "disabled", "disallow"];

    NEGATIVE_PREFIXES.iter().any(|p| name.starts_with(p)) || NEGATIVE_EXACT_WORDS.contains(&name)
}

/// Determines if a test function name begins with a forbidden `test_` or `test` prefix.
pub fn check_ident_has_test_prefix(name: &str) -> bool {
    name.starts_with("test_")
        || name == "test"
        || (name.starts_with("test")
            && name
                .chars()
                .nth(4)
                .map(|c| c.is_ascii_uppercase() || c == '_')
                .unwrap_or(false))
}
