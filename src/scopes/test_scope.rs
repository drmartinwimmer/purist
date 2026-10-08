//! Test scope tracking across modules and functions during AST traversal using a stack.

use super::guard::run_with_scope;
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
pub struct TestScope {
    stack: Vec<TestScopeState>,
}

impl TestScope {
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

    /// Executes a closure within an entered module scope.
    pub fn with_mod<R>(&mut self, attrs: &[Attribute], f: impl FnOnce(&mut Self) -> R) -> R {
        run_with_scope(self, |t| t.push_mod(attrs), |t| t.pop(), f)
    }

    /// Executes a closure within an entered function scope.
    pub fn with_fn<R>(&mut self, attrs: &[Attribute], f: impl FnOnce(&mut Self) -> R) -> R {
        run_with_scope(self, |t| t.push_fn(attrs), |t| t.pop(), f)
    }

    /// Executes a closure within an entered function scope with the specified name.
    pub fn with_fn_with_name<R>(
        &mut self,
        attrs: &[Attribute],
        fn_name: &str,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        run_with_scope(
            self,
            |t| t.push_fn_with_name(attrs, fn_name),
            |t| t.pop(),
            f,
        )
    }
}

/// Trait for visitor types that hold a [`TestScope`], providing scoped closure methods.
pub trait WithTestScope {
    /// Returns a mutable reference to the underlying test scope.
    fn test_scope_mut(&mut self) -> &mut TestScope;

    /// Runs a closure within an entered module scope.
    fn with_test_mod<R>(&mut self, attrs: &[Attribute], f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.test_scope_mut().push_mod(attrs),
            |v| v.test_scope_mut().pop(),
            f,
        )
    }

    /// Runs a closure within an entered function scope.
    fn with_test_fn<R>(&mut self, attrs: &[Attribute], f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.test_scope_mut().push_fn(attrs),
            |v| v.test_scope_mut().pop(),
            f,
        )
    }

    /// Runs a closure within an entered function scope with a specific name.
    fn with_test_fn_with_name<R>(
        &mut self,
        attrs: &[Attribute],
        fn_name: &str,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.test_scope_mut().push_fn_with_name(attrs, fn_name),
            |v| v.test_scope_mut().pop(),
            f,
        )
    }
}

impl WithTestScope for TestScope {
    fn test_scope_mut(&mut self) -> &mut TestScope {
        self
    }
}
