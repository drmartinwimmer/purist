//! RAII scope guards for AST visitor state restoration using scoped closures.

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
