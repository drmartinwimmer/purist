//! Parameterized checkers for macro invocations.

use syn::Macro;

/// Checks whether a macro invocation matches any of the target names.
///
/// Matches against the last path segment of the macro path (e.g. `assert_eq` in `std::assert_eq!`).
pub fn check_macro_matches(mac: &Macro, target_names: &[&str]) -> Option<String> {
    let name = mac.path.segments.last()?.ident.to_string();
    if target_names.iter().any(|target| *target == name) {
        Some(name)
    } else {
        None
    }
}
