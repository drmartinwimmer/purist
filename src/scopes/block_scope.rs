//! Lexical block scope tracker maintaining a stack of variable and symbol bindings.

use super::guard::run_with_scope;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

/// Lexical block scope tracking symbol or variable mappings across nested AST blocks using a stack (`Vec`).
#[derive(Debug, Clone, Default)]
pub struct BlockScope<K, V> {
    stack: Vec<HashMap<K, V>>,
}

impl<K: Eq + Hash, V> BlockScope<K, V> {
    /// Creates a new empty block scope.
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Returns the current nesting depth of block scopes.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Returns true if no block scopes are currently pushed.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// Pushes a new block scope with variable bindings onto the stack.
    pub fn push(&mut self, bindings: HashMap<K, V>) {
        self.stack.push(bindings);
    }

    /// Pops the innermost block scope from the stack.
    pub fn pop(&mut self) -> Option<HashMap<K, V>> {
        self.stack.pop()
    }

    /// Looks up a key, searching from the innermost block scope outward.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        for block in self.stack.iter().rev() {
            if let Some(val) = block.get(key) {
                return Some(val);
            }
        }
        None
    }

    /// Returns true if any active block scope contains the key.
    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.get(key).is_some()
    }

    /// Executes a closure within an entered block scope, ensuring it is popped on exit.
    pub fn with_block<R>(&mut self, bindings: HashMap<K, V>, f: impl FnOnce(&mut Self) -> R) -> R {
        run_with_scope(
            self,
            |s| s.push(bindings),
            |s| {
                s.pop();
            },
            f,
        )
    }
}

/// Trait for visitor types that hold a [`BlockScope`], providing scoped closure methods.
pub trait WithBlockScope<K, V> {
    /// Returns a mutable reference to the underlying block scope.
    fn block_scope_mut(&mut self) -> &mut BlockScope<K, V>;

    /// Runs a closure within an entered lexical block scope, popping on exit.
    fn with_block<R>(&mut self, bindings: HashMap<K, V>, f: impl FnOnce(&mut Self) -> R) -> R
    where
        K: Eq + Hash,
        Self: Sized,
    {
        run_with_scope(
            self,
            |v| v.block_scope_mut().push(bindings),
            |v| {
                v.block_scope_mut().pop();
            },
            f,
        )
    }
}

impl<K, V> WithBlockScope<K, V> for BlockScope<K, V> {
    fn block_scope_mut(&mut self) -> &mut BlockScope<K, V> {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn block_scope_tracks_bindings_and_shadowing() {
        let mut scope = BlockScope::new();
        assert_that!(scope.is_empty(), eq(true));
        assert_that!(scope.depth(), eq(0));
        assert_that!(scope.get("x"), none());
        assert_that!(scope.contains("x"), eq(false));

        let mut level1 = HashMap::new();
        level1.insert("x".to_string(), 10);
        level1.insert("y".to_string(), 20);

        scope.with_block(level1, |s1| {
            assert_that!(s1.is_empty(), eq(false));
            assert_that!(s1.depth(), eq(1));
            assert_that!(s1.get("x"), some(eq(&10)));
            assert_that!(s1.get("y"), some(eq(&20)));
            assert_that!(s1.contains("x"), eq(true));

            let mut level2 = HashMap::new();
            level2.insert("x".to_string(), 99); // shadow x
            level2.insert("z".to_string(), 30);

            s1.with_block(level2, |s2| {
                assert_that!(s2.depth(), eq(2));
                assert_that!(s2.get("x"), some(eq(&99))); // shadowed
                assert_that!(s2.get("y"), some(eq(&20))); // outer
                assert_that!(s2.get("z"), some(eq(&30))); // inner
            });

            assert_that!(s1.depth(), eq(1));
            assert_that!(s1.get("x"), some(eq(&10))); // restored
            assert_that!(s1.get("z"), none());
        });

        assert_that!(scope.is_empty(), eq(true));
        assert_that!(scope.depth(), eq(0));
        assert_that!(scope.get("x"), none());
    }
}
