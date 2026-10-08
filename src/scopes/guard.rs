//! RAII scope guards for AST visitor state restoration using scoped closures.

use super::block_scope::BlockScope;
use super::depth_scope::FlagScope;
use std::collections::HashMap;
use std::hash::Hash;

/// RAII scope guard ensuring a pop or cleanup action runs upon normal completion or unwind.
pub struct ScopeGuard<'a, V, P: FnOnce(&mut V)> {
    visitor: &'a mut V,
    pop: Option<P>,
}

impl<'a, V, P: FnOnce(&mut V)> Drop for ScopeGuard<'a, V, P> {
    fn drop(&mut self) {
        if let Some(pop) = self.pop.take() {
            pop(self.visitor);
        }
    }
}

/// Executes a closure within a scoped context, ensuring the pop callback is invoked on drop.
pub fn run_with_scope<V, P, R>(
    visitor: &mut V,
    push: impl FnOnce(&mut V),
    pop: P,
    f: impl FnOnce(&mut V) -> R,
) -> R
where
    P: FnOnce(&mut V),
{
    push(visitor);
    let guard = ScopeGuard {
        visitor,
        pop: Some(pop),
    };
    let res = f(guard.visitor);
    drop(guard);
    res
}

/// Runs a closure with a flag scope on the target updated monotonically via RAII.
pub fn run_with_flag<V, R>(
    target: &mut V,
    flag_fn: impl Fn(&mut V) -> &mut FlagScope,
    condition: bool,
    f: impl FnOnce(&mut V) -> R,
) -> R {
    run_with_scope(
        target,
        |v| flag_fn(v).push(condition),
        |v| {
            flag_fn(v).pop();
        },
        f,
    )
}

/// Runs a closure with an exact flag state pushed onto the target's flag scope via RAII.
pub fn run_with_exact_flag<V, R>(
    target: &mut V,
    flag_fn: impl Fn(&mut V) -> &mut FlagScope,
    condition: bool,
    f: impl FnOnce(&mut V) -> R,
) -> R {
    run_with_scope(
        target,
        |v| flag_fn(v).push_exact(condition),
        |v| {
            flag_fn(v).pop();
        },
        f,
    )
}

/// Runs a closure with a lexical block scope on the target pushed and popped via RAII.
pub fn run_with_block<V, K, Val, R>(
    target: &mut V,
    block_fn: impl Fn(&mut V) -> &mut BlockScope<K, Val>,
    bindings: HashMap<K, Val>,
    f: impl FnOnce(&mut V) -> R,
) -> R
where
    K: Eq + Hash,
{
    run_with_scope(
        target,
        |v| block_fn(v).push(bindings),
        |v| {
            block_fn(v).pop();
        },
        f,
    )
}
