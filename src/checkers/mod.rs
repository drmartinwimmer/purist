//! # AST Checkers
//!
//! Reusable, parameterized AST checkers used across Purist rules:
//! - [`call`]: Matchers for function calls and method invocations.
//! - [`signature`]: Checkers for function parameters, receivers, and return types.
//! - [`macro_check`]: Checkers for macro invocations.
//! - [`naming`]: Checkers for identifier naming patterns and prefixes.

pub(crate) mod call;
pub(crate) mod macro_check;
pub(crate) mod naming;
pub(crate) mod signature;

pub(crate) use call::{check_call_matches_path, check_method_call_matches_name};
pub(crate) use macro_check::check_macro_matches;
pub(crate) use naming::check_ident_has_test_prefix;
pub(crate) use signature::{ReceiverKind, check_fn_receiver, extract_fn_return_type};
