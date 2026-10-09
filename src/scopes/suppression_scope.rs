//! Scope tracker for attribute-level rule suppressions during AST traversal.

use super::guard::run_with_scope;

/// Tracks whether AST traversal is currently inside an `#[allow(...)]` or `#[expect(...)]` block.
#[derive(Debug, Clone, Default)]
pub struct SuppressionScope {
    stack: Vec<bool>,
}

impl SuppressionScope {
    /// Creates a new suppression scope tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if currently traversing inside a suppressed region.
    pub fn is_suppressed(&self) -> bool {
        self.stack.last().copied().unwrap_or(false)
    }

    /// Pushes a suppression flag onto the stack. If already suppressed, child scopes stay suppressed.
    pub fn push_suppression(&mut self, suppressed: bool) {
        let active = suppressed || self.is_suppressed();
        self.stack.push(active);
    }

    /// Pops the active suppression scope from the stack.
    pub fn pop(&mut self) -> Option<bool> {
        self.stack.pop()
    }

    /// Executes a closure within this suppression scope, popping on exit.
    pub fn with_suppression<R>(&mut self, suppressed: bool, f: impl FnOnce(&mut Self) -> R) -> R {
        run_with_scope(
            self,
            |t| t.push_suppression(suppressed),
            |t| {
                t.pop();
            },
            f,
        )
    }
}

/// Trait for visitor types that hold a [`SuppressionScope`], providing scoped closure methods.
pub trait WithSuppressionScope {
    /// Returns a mutable reference to the underlying suppression scope.
    fn suppression_scope_mut(&mut self) -> &mut SuppressionScope;

    /// Runs a closure within an entered suppression scope.
    fn with_suppression<R>(&mut self, suppressed: bool, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.suppression_scope_mut().push_suppression(suppressed),
            |v| {
                v.suppression_scope_mut().pop();
            },
            f,
        )
    }
}

impl WithSuppressionScope for SuppressionScope {
    fn suppression_scope_mut(&mut self) -> &mut SuppressionScope {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn suppression_scope_tracks_suppression_and_inheritance() {
        let mut scope = SuppressionScope::new();
        assert_that!(scope.is_suppressed(), eq(false));

        scope.with_suppression(true, |s| {
            assert_that!(s.is_suppressed(), eq(true));

            // Nested scope stays suppressed even if condition is false
            s.with_suppression(false, |s2| {
                assert_that!(s2.is_suppressed(), eq(true));
            });

            assert_that!(s.is_suppressed(), eq(true));
        });

        assert_that!(scope.is_suppressed(), eq(false));
    }
}
