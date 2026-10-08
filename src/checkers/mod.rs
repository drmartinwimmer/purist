//! # AST Checkers
//!
//! Reusable, parameterized AST checkers used across Purist rules:
//! - [`call`]: Matchers for function calls and method invocations.
//! - [`signature`]: Checkers for function parameters, receivers, and return types.
//! - [`macro_check`]: Checkers for macro invocations.
//! - [`naming`]: Checkers for identifier naming patterns and prefixes.

pub mod call;
pub mod macro_check;
pub mod naming;
pub mod signature;

pub use call::{
    check_call_matches_path, check_expr_matches_path, check_method_call_matches_name,
    check_path_matches,
};
pub use macro_check::check_macro_matches;
pub use naming::{
    check_ident_has_negative_name, check_ident_has_prefix, check_ident_has_test_prefix,
};
pub use signature::{
    ReceiverKind, check_fn_receiver, extract_fn_return_type, extract_result_error_type,
};
