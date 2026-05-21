//! Spike: CS-trait architecture (issue #106).
//!
//! **Throwaway, exploratory code.** This module reorganizes one end-to-end
//! slice of the KenKen solver around standard constraint-satisfaction
//! nomenclature ([`Variable`](variable::Variable),
//! [`Constraint`](constraint::Constraint), [`Solver`](solver::Solver)) layered
//! over the existing domain types (`Cell`, `Polyomino`, `Operation`, `Fill`).
//! Its purpose is to be a concrete comparison partner to the parallel Pumpkin
//! spike — not to ship.
//!
//! The headline architectural point is the separation of **store** (intrinsic
//! variable domains; see [`store::Store`]) from **cache** (derived viable-tuple
//! sets; see [`cache`]). Viable tuples are obtained exclusively through the
//! pure, memoized function [`cache::viable_tuples`]; the spike's [`cage::CageDef`]
//! carries no mutable tuple state, which structurally prevents the historical
//! `Cage.tuples` smell from recurring.

// `pub` items below trip module_name_repetitions on the CS vocabulary
// (e.g. `AllDifferentPropagator` in `all_different`). The repetition is
// deliberate — it mirrors the standard nomenclature the spike is exploring.
#![allow(clippy::module_name_repetitions)]

mod all_different;
mod cache;
mod cage;
mod constraint;
mod fixtures;
mod problem;
mod solver;
mod store;
mod variable;

#[cfg(test)]
mod tests;

use all_different::AllDiffDef;
use cage::CageDef;
use constraint::{Constraint, Outcome, PropagationCtx};

use crate::Cell;

/// The single concrete constraint type carried across the KenKen slice.
///
/// [`constraint::propagate_to_fixpoint`] is generic over one homogeneous
/// constraint type `C`, but a KenKen problem mixes cages and all-different
/// constraints. This enum is that one type: it dispatches the
/// [`Constraint`] trait to whichever variant it holds. Rows and columns both
/// use [`AllDiffDef`] (they differ only in which cells they cover), so a single
/// `AllDiff` variant suffices.
#[derive(Debug, Clone)]
pub enum KenKenConstraint {
    Cage(CageDef),
    AllDiff(AllDiffDef),
}

impl Constraint<Cell> for KenKenConstraint {
    fn variables(&self) -> &[Cell] {
        match self {
            Self::Cage(c) => c.variables(),
            Self::AllDiff(a) => a.variables(),
        }
    }

    fn propagate(&self, ctx: &mut PropagationCtx<Cell>) -> Outcome {
        match self {
            Self::Cage(c) => c.propagate(ctx),
            Self::AllDiff(a) => a.propagate(ctx),
        }
    }
}
