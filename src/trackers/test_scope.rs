//! Test scope tracking across modules and functions during AST traversal.

use super::guard::ScopeGuard;
use crate::rules::common::{has_cfg_test_attr, has_test_attr};
use std::cell::Cell;
use std::rc::Rc;
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
/// during AST traversal using RAII guards.
#[derive(Debug, Clone, Default)]
pub struct TestScopeTracker {
    state: Rc<Cell<TestScopeState>>,
}

impl TestScopeTracker {
    /// Creates a new test scope tracker, initialized with whether the current file is a test file.
    pub fn new(is_test_file: bool) -> Self {
        Self {
            state: Rc::new(Cell::new(TestScopeState {
                in_test_module: is_test_file,
                in_test_fn: false,
            })),
        }
    }

    /// Returns true if currently within any test context (test file, `#[cfg(test)]` module, or test function).
    pub fn is_in_test(&self) -> bool {
        self.state.get().is_in_test()
    }

    /// Returns true if currently within a `#[cfg(test)]` module or a dedicated test file.
    pub fn is_in_test_module(&self) -> bool {
        self.state.get().in_test_module
    }

    /// Returns true if currently within the body of a test function.
    pub fn is_in_test_fn(&self) -> bool {
        self.state.get().in_test_fn
    }

    /// Updates the tracker when entering a module with attributes, returning an RAII guard
    /// that restores the previous state when dropped.
    pub fn enter_mod(&self, attrs: &[Attribute]) -> ScopeGuard<TestScopeState> {
        let prev = self.state.get();
        let mut next = prev;
        if has_cfg_test_attr(attrs) {
            next.in_test_module = true;
        }
        self.state.set(next);
        ScopeGuard::new(Rc::clone(&self.state), prev)
    }

    /// Updates the tracker when entering a function, returning an RAII guard
    /// that restores the previous state when dropped.
    pub fn enter_fn(&self, attrs: &[Attribute]) -> ScopeGuard<TestScopeState> {
        let prev = self.state.get();
        let mut next = prev;
        if has_test_attr(attrs) {
            next.in_test_fn = true;
        }
        self.state.set(next);
        ScopeGuard::new(Rc::clone(&self.state), prev)
    }

    /// Updates the tracker when entering a function, also treating functions in test modules
    /// starting with `test_` as test functions. Returns an RAII guard that restores the previous state.
    pub fn enter_fn_with_name(
        &self,
        attrs: &[Attribute],
        fn_name: &str,
    ) -> ScopeGuard<TestScopeState> {
        let prev = self.state.get();
        let mut next = prev;
        if has_test_attr(attrs) || (prev.in_test_module && fn_name.starts_with("test_")) {
            next.in_test_fn = true;
        }
        self.state.set(next);
        ScopeGuard::new(Rc::clone(&self.state), prev)
    }
}
