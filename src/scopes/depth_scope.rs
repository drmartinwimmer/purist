//! Scopes for numeric depth and boolean flags using stacks.

use super::guard::run_with_scope;

/// Scope for measuring nesting depth during AST traversal.
#[derive(Debug, Clone, Default)]
pub struct DepthScope {
    depth: usize,
    stack: Vec<usize>,
}

impl DepthScope {
    /// Creates a new depth scope initialized to 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current nesting depth.
    pub fn get(&self) -> usize {
        self.depth
    }

    /// Increments the current nesting depth.
    pub fn enter(&mut self) -> usize {
        self.depth += 1;
        self.depth
    }

    /// Decrements the current nesting depth.
    pub fn exit(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// Pushes the current depth onto a stack and resets depth to 0 (e.g. upon entering a function or closure).
    pub fn push_root(&mut self) {
        self.stack.push(self.depth);
        self.depth = 0;
    }

    /// Pops the previous depth from the stack when leaving a function or closure.
    pub fn pop_root(&mut self) {
        if let Some(prev) = self.stack.pop() {
            self.depth = prev;
        }
    }

    /// Executes a closure within an entered root depth scope (depth reset to 0).
    pub fn with_root<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        run_with_scope(self, |t| t.push_root(), |t| t.pop_root(), f)
    }

    /// Executes a closure at an incremented nesting depth.
    pub fn with_depth<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        run_with_scope(
            self,
            |t| {
                t.enter();
            },
            |t| t.exit(),
            f,
        )
    }
}

/// Trait for visitor types that hold a [`DepthScope`], providing scoped closure methods.
pub trait WithDepthScope {
    /// Returns a mutable reference to the underlying depth scope.
    fn depth_scope_mut(&mut self) -> &mut DepthScope;

    /// Runs a closure with depth reset to 0 (root scope).
    fn with_depth_root<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.depth_scope_mut().push_root(),
            |v| v.depth_scope_mut().pop_root(),
            f,
        )
    }

    /// Runs a closure with an incremented depth level.
    fn with_depth_step<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R
    where
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| {
                v.depth_scope_mut().enter();
            },
            |v| v.depth_scope_mut().exit(),
            f,
        )
    }
}

impl WithDepthScope for DepthScope {
    fn depth_scope_mut(&mut self) -> &mut DepthScope {
        self
    }
}

/// Scope for boolean flags during AST traversal using a stack (`Vec`).
#[derive(Debug, Clone, Default)]
pub struct FlagScope {
    stack: Vec<bool>,
}

impl FlagScope {
    /// Creates a new flag scope.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if the flag scope is currently active.
    pub fn is_active(&self) -> bool {
        self.stack.last().copied().unwrap_or(false)
    }

    /// Pushes a new flag state onto the stack. If `condition` is true or if already active, stays active.
    pub fn push(&mut self, condition: bool) {
        let next = condition || self.is_active();
        self.stack.push(next);
    }

    /// Pops the active flag scope from the stack.
    pub fn pop(&mut self) -> Option<bool> {
        self.stack.pop()
    }

    /// Executes a closure within this flag scope, popping on exit.
    pub fn with_flag<R>(&mut self, condition: bool, f: impl FnOnce(&mut Self) -> R) -> R {
        run_with_scope(
            self,
            |t| t.push(condition),
            |t| {
                t.pop();
            },
            f,
        )
    }
}
