//! RAII scope guards for AST visitor state restoration.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// RAII scope guard for trackers with `Copy` state stored in a `Cell`.
///
/// When the guard goes out of scope, it automatically restores the tracker's state
/// to the state captured prior to entering the scope.
#[must_use = "scope guard must be held in a local variable to maintain scope"]
pub struct ScopeGuard<T: Copy> {
    state: Rc<Cell<T>>,
    prev: T,
}

impl<T: Copy> ScopeGuard<T> {
    /// Creates a new RAII scope guard capturing the previous state.
    pub fn new(state: Rc<Cell<T>>, prev: T) -> Self {
        Self { state, prev }
    }

    /// Explicitly restores the previous state before drop, consuming the guard.
    pub fn restore(self) {
        drop(self);
    }
}

impl<T: Copy> Drop for ScopeGuard<T> {
    fn drop(&mut self) {
        self.state.set(self.prev);
    }
}

/// RAII scope guard for trackers with state stored in a `RefCell`.
///
/// When the guard goes out of scope, it automatically restores the tracker's state
/// to the state captured prior to entering the scope.
#[must_use = "scope guard must be held in a local variable to maintain scope"]
pub struct RefScopeGuard<T> {
    state: Rc<RefCell<T>>,
    prev: Option<T>,
}

impl<T> RefScopeGuard<T> {
    /// Creates a new RAII scope guard capturing the previous state.
    pub fn new(state: Rc<RefCell<T>>, prev: T) -> Self {
        Self {
            state,
            prev: Some(prev),
        }
    }
}

impl<T> Drop for RefScopeGuard<T> {
    fn drop(&mut self) {
        if let Some(prev) = self.prev.take() {
            *self.state.borrow_mut() = prev;
        }
    }
}
