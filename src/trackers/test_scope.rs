//! Test scope tracking across modules and functions during AST traversal using a stack.

use crate::rules::common::{has_cfg_test_attr, has_test_attr};
use syn::Attribute;

/// Snapshot state of the test scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TestScopeState {
    pub in_test_module: bool,
    pub in_test_fn: bool,
}

impl TestScopeState {
    /// Returns true if currently within any test context (module or test function).
    pub fn is_in_test(&self) -> bool {
        self.in_test_module || self.in_test_fn
    }
}

/// Tracks lexical test scopes (test files, `#[cfg(test)]` modules, and test functions)
/// during AST traversal using a scope stack (`Vec`).
#[derive(Debug, Clone)]
pub struct TestScopeTracker {
    stack: Vec<TestScopeState>,
}

impl TestScopeTracker {
    /// Creates a new test scope tracker, initialized with whether the current file is a test file.
    pub fn new(is_test_file: bool) -> Self {
        Self {
            stack: vec![TestScopeState {
                in_test_module: is_test_file,
                in_test_fn: false,
            }],
        }
    }

    fn current(&self) -> TestScopeState {
        self.stack.last().copied().unwrap_or_default()
    }

    /// Returns true if currently within any test context (test file, `#[cfg(test)]` module, or test function).
    pub fn is_in_test(&self) -> bool {
        self.current().is_in_test()
    }

    /// Returns true if currently within a `#[cfg(test)]` module or a dedicated test file.
    pub fn is_in_test_module(&self) -> bool {
        self.current().in_test_module
    }

    /// Returns true if currently within the body of a test function.
    pub fn is_in_test_fn(&self) -> bool {
        self.current().in_test_fn
    }

    /// Pushes a new module scope onto the stack.
    pub fn push_mod(&mut self, attrs: &[Attribute]) {
        let prev = self.current();
        let mut next = prev;
        if has_cfg_test_attr(attrs) {
            next.in_test_module = true;
        }
        self.stack.push(next);
    }

    /// Pushes a new function scope onto the stack.
    pub fn push_fn(&mut self, attrs: &[Attribute]) {
        let prev = self.current();
        let mut next = prev;
        if has_test_attr(attrs) {
            next.in_test_fn = true;
        }
        self.stack.push(next);
    }

    /// Pushes a new function scope onto the stack, also treating functions in test modules
    /// starting with `test_` as test functions.
    pub fn push_fn_with_name(&mut self, attrs: &[Attribute], fn_name: &str) {
        let prev = self.current();
        let mut next = prev;
        if has_test_attr(attrs) || (prev.in_test_module && fn_name.starts_with("test_")) {
            next.in_test_fn = true;
        }
        self.stack.push(next);
    }

    /// Pops the active scope from the stack.
    pub fn pop(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }
}
