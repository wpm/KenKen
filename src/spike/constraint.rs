//! The CS-nomenclature [`Constraint`] trait, the [`PropagationCtx`] it mutates,
//! and the reusable [`propagate_to_fixpoint`] free function.

use std::marker::PhantomData;

use crate::spike::{cache::Cache, store::Store, variable::Variable};

/// What a single propagation step did to the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// No domain changed.
    Unchanged,
    /// At least one domain was narrowed.
    Changed,
    /// A domain became empty — the (sub)problem is infeasible.
    Contradiction,
}

/// The mutable context a constraint propagates against.
///
/// It holds the [`Store`] (intrinsic state being narrowed) and the [`Cache`]
/// (derived viable-tuple memo) as two *separate* mutable borrows, so a
/// constraint can read a cached tuple set and write domain reductions back to
/// the store without aliasing — the store and cache never overlap.
pub struct PropagationCtx<'a, V: Variable> {
    pub store: &'a mut Store,
    pub cache: &'a mut Cache,
    marker: PhantomData<V>,
}

impl<'a, V: Variable> PropagationCtx<'a, V> {
    pub const fn new(store: &'a mut Store, cache: &'a mut Cache) -> Self {
        Self {
            store,
            cache,
            marker: PhantomData,
        }
    }
}

/// A constraint over variables of type `V`.
pub trait Constraint<V: Variable> {
    /// The variables this constraint ranges over.
    fn variables(&self) -> &[V];
    /// Narrows the domains in `ctx` toward consistency, reporting the effect.
    fn propagate(&self, ctx: &mut PropagationCtx<V>) -> Outcome;
}

/// Applies every constraint repeatedly until no domain changes (a fixed point)
/// or a contradiction is found.
///
/// A free function rather than a [`Solver`](crate::spike::solver::Solver)
/// method, because propagation is useful outside search: the Designer needs the
/// reduced store ([`fixpoint`](crate::spike::problem::fixpoint)) without
/// committing to a full solve.
pub fn propagate_to_fixpoint<V, C>(ctx: &mut PropagationCtx<V>, constraints: &[C]) -> Outcome
where
    V: Variable,
    C: Constraint<V>,
{
    let mut overall = Outcome::Unchanged;
    loop {
        let mut changed = false;
        for constraint in constraints {
            match constraint.propagate(ctx) {
                Outcome::Contradiction => return Outcome::Contradiction,
                Outcome::Changed => {
                    changed = true;
                    overall = Outcome::Changed;
                }
                Outcome::Unchanged => {}
            }
        }
        if !changed {
            return overall;
        }
    }
}
