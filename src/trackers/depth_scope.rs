//! Scope trackers for numeric depth and boolean flags using stacks.

/// Tracker for measuring nesting depth during AST traversal.
#[derive(Debug, Clone, Default)]
pub struct DepthTracker {
    depth: usize,
    stack: Vec<usize>,
}

impl DepthTracker {
    /// Creates a new depth tracker initialized to 0.
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
}

/// Tracker for boolean flag scopes during AST traversal using a stack (`Vec`).
#[derive(Debug, Clone, Default)]
pub struct FlagScopeTracker {
    stack: Vec<bool>,
}

impl FlagScopeTracker {
    /// Creates a new flag scope tracker.
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
}
