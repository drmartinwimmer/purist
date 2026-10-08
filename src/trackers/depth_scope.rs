//! Scope trackers for numeric depth and boolean flags.

use super::guard::ScopeGuard;
use std::cell::Cell;
use std::rc::Rc;

/// RAII tracker for measuring nesting depth during AST traversal.
#[derive(Debug, Clone, Default)]
pub struct DepthTracker {
    depth: Rc<Cell<usize>>,
}

impl DepthTracker {
    /// Creates a new depth tracker initialized to 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current nesting depth.
    pub fn get(&self) -> usize {
        self.depth.get()
    }

    /// Resets depth to 0 (e.g. upon entering a function or closure boundary)
    /// and returns an RAII guard that restores the previous depth on drop.
    pub fn reset(&self) -> ScopeGuard<usize> {
        let prev = self.depth.get();
        self.depth.set(0);
        ScopeGuard::new(Rc::clone(&self.depth), prev)
    }

    /// Increments nesting depth by 1 and returns an RAII guard that restores the previous depth on drop.
    pub fn enter(&self) -> ScopeGuard<usize> {
        let prev = self.depth.get();
        self.depth.set(prev + 1);
        ScopeGuard::new(Rc::clone(&self.depth), prev)
    }
}

/// RAII tracker for boolean flag scopes during AST traversal.
#[derive(Debug, Clone, Default)]
pub struct FlagScopeTracker {
    state: Rc<Cell<bool>>,
}

impl FlagScopeTracker {
    /// Creates a new flag scope tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if the flag scope is currently active.
    pub fn is_active(&self) -> bool {
        self.state.get()
    }

    /// Activates the flag if `condition` is true, returning an RAII guard that restores previous state on drop.
    pub fn enter(&self, condition: bool) -> ScopeGuard<bool> {
        let prev = self.state.get();
        if condition {
            self.state.set(true);
        }
        ScopeGuard::new(Rc::clone(&self.state), prev)
    }
}
