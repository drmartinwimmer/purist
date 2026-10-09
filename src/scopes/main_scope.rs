//! Scope tracker for `fn main()` entrypoint boundaries during AST traversal.

use super::guard::run_with_scope;

/// Tracks whether AST traversal is currently inside `fn main()` in `main.rs`.
#[derive(Debug, Clone, Default)]
pub struct MainScope {
    stack: Vec<bool>,
}

impl MainScope {
    /// Creates a new main function scope tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if currently traversing inside `fn main()`.
    pub fn is_in_main(&self) -> bool {
        self.stack.last().copied().unwrap_or(false)
    }

    /// Pushes a main function entrypoint flag onto the stack.
    pub fn push_main(&mut self, is_main: bool) {
        self.stack.push(is_main);
    }

    /// Pops the active main function scope from the stack.
    pub fn pop(&mut self) -> Option<bool> {
        self.stack.pop()
    }

    /// Executes a closure within this main function scope, popping on exit.
    pub fn with_main_fn<R>(&mut self, is_main: bool, f: impl FnOnce(&mut Self) -> R) -> R {
        run_with_scope(
            self,
            |t| t.push_main(is_main),
            |t| {
                t.pop();
            },
            f,
        )
    }
}

/// Trait for visitor types that hold a [`MainScope`], providing scoped closure methods.
pub trait WithMainScope {
    /// Returns a mutable reference to the underlying main function scope.
    fn main_scope_mut(&mut self) -> &mut MainScope;

    /// Runs a closure within an entered main function scope.
    fn with_main_fn<R>(&mut self, is_main: bool, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.main_scope_mut().push_main(is_main),
            |v| {
                v.main_scope_mut().pop();
            },
            f,
        )
    }
}

impl WithMainScope for MainScope {
    fn main_scope_mut(&mut self) -> &mut MainScope {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn main_scope_tracks_entrypoint() {
        let mut scope = MainScope::new();
        assert_that!(scope.is_in_main(), eq(false));

        scope.with_main_fn(true, |s| {
            assert_that!(s.is_in_main(), eq(true));
        });

        assert_that!(scope.is_in_main(), eq(false));
    }
}
