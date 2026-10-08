//! Parameterized checkers for function and method call expressions.

use syn::{Expr, ExprCall, ExprMethodCall, Path};

/// Checks whether an expression is a path ending with any of the target segment slices.
pub fn check_expr_matches_path(expr: &Expr, targets: &[&[&str]]) -> bool {
    match expr {
        Expr::Path(expr_path) => check_path_matches(&expr_path.path, targets),
        _ => false,
    }
}

/// Checks whether a function call expression matches any of the specified path segment patterns.
///
/// For example, `targets` might be `&[&["std", "process", "Command", "new"], &["Command", "new"]]`.
pub fn check_call_matches_path(call: &ExprCall, targets: &[&[&str]]) -> bool {
    check_expr_matches_path(&call.func, targets)
}

/// Checks whether a syn::Path ends with any of the specified segment slices.
pub fn check_path_matches(path: &Path, targets: &[&[&str]]) -> bool {
    let segments: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    targets.iter().any(|target| {
        if target.len() > segments.len() {
            return false;
        }
        let offset = segments.len() - target.len();
        segments.get(offset..).is_some_and(|suffix| {
            suffix
                .iter()
                .zip(target.iter())
                .all(|(seg, tgt)| seg == tgt)
        })
    })
}

/// Checks whether a method call expression invokes one of the target method names.
pub fn check_method_call_matches_name(call: &ExprMethodCall, target_methods: &[&str]) -> bool {
    let method_name = call.method.to_string();
    target_methods.iter().any(|target| *target == method_name)
}
