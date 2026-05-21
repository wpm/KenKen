//! Problem assembly and the Designer-facing [`fixpoint`] entry point.

use std::marker::PhantomData;

use crate::{
    Cell, Fill,
    spike::{
        KenKenConstraint,
        all_different::AllDiffDef,
        cache::Cache,
        cage::CageDef,
        constraint::{Constraint, PropagationCtx, propagate_to_fixpoint},
        store::Store,
        variable::Variable,
    },
};

/// A constraint-satisfaction problem: an initial store plus a set of constraints.
pub struct Problem<V: Variable, C: Constraint<V>> {
    initial_store: Store,
    constraints: Vec<C>,
    marker: PhantomData<V>,
}

impl<V: Variable, C: Constraint<V>> Problem<V, C> {
    pub fn new(initial_store: Store, constraints: Vec<C>) -> Self {
        Self {
            initial_store,
            constraints,
            marker: PhantomData,
        }
    }

    pub const fn initial_store(&self) -> &Store {
        &self.initial_store
    }

    pub fn constraints(&self) -> &[C] {
        &self.constraints
    }
}

/// A partial KenKen: a grid size, the cages placed so far, and any committed
/// cell domains ("givens"). This is the in-progress state the Designer works
/// with; [`fixpoint`] reduces it without search.
#[derive(Debug, Clone)]
pub struct PartialPuzzle {
    n: usize,
    cages: Vec<CageDef>,
    givens: Vec<(Cell, Fill)>,
}

impl PartialPuzzle {
    pub fn new(n: usize) -> Self {
        Self {
            n,
            cages: vec![],
            givens: vec![],
        }
    }

    pub fn with_cage(mut self, cage: CageDef) -> Self {
        self.cages.push(cage);
        self
    }

    pub fn with_given(mut self, cell: Cell, domain: Fill) -> Self {
        self.givens.push((cell, domain));
        self
    }

    /// The intrinsic starting state: full domains, narrowed by the givens.
    pub fn store(&self) -> Store {
        let mut store = Store::full(self.n);
        for &(cell, domain) in &self.givens {
            let _ = store.intersect(cell.id(), domain);
        }
        store
    }

    /// All constraints: a row and a column all-different per index, plus a
    /// constraint per placed cage.
    pub fn constraints(&self) -> Vec<KenKenConstraint> {
        let mut constraints: Vec<KenKenConstraint> = Vec::new();
        for i in 0..self.n {
            constraints.push(KenKenConstraint::AllDiff(AllDiffDef::row(self.n, i)));
            constraints.push(KenKenConstraint::AllDiff(AllDiffDef::column(self.n, i)));
        }
        for cage in &self.cages {
            constraints.push(KenKenConstraint::Cage(cage.clone()));
        }
        constraints
    }

    pub fn problem(&self) -> Problem<Cell, KenKenConstraint> {
        Problem::new(self.store(), self.constraints())
    }
}

/// Runs propagation to a fixed point without search and returns the reduced
/// store. This is what the Designer's tuple-highlight feature will call.
pub fn fixpoint(partial: &PartialPuzzle) -> Store {
    let mut store = partial.store();
    let constraints = partial.constraints();
    let mut cache = Cache::default();
    let mut ctx = PropagationCtx::new(&mut store, &mut cache);
    let _ = propagate_to_fixpoint(&mut ctx, &constraints);
    store
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Operation, Polyomino};

    #[test]
    fn store_applies_givens() {
        let partial = PartialPuzzle::new(4).with_given(Cell::new(0, 0), Fill::new([2]));
        assert_eq!(partial.store().get(Cell::new(0, 0).id()), Fill::new([2]));
    }

    #[test]
    fn constraints_include_rows_columns_and_cages() {
        let poly = Polyomino::from_cells(&[Cell::new(0, 0)]).unwrap();
        let partial = PartialPuzzle::new(4).with_cage(CageDef::new(4, poly, Operation::Given(3)));
        // 4 rows + 4 columns + 1 cage.
        assert_eq!(partial.constraints().len(), 9);
        assert_eq!(partial.problem().constraints().len(), 9);
    }

    #[test]
    fn fixpoint_propagates_a_given_cage() {
        let poly = Polyomino::from_cells(&[Cell::new(0, 0)]).unwrap();
        let partial = PartialPuzzle::new(4).with_cage(CageDef::new(4, poly, Operation::Given(3)));
        let store = fixpoint(&partial);
        // The Given(3) pins (0,0); all-different removes 3 from the rest of row 0
        // and column 0.
        assert_eq!(store.get(Cell::new(0, 0).id()), Fill::new([3]));
        assert_eq!(store.get(Cell::new(0, 1).id()), Fill::new([1, 2, 4]));
        assert_eq!(store.get(Cell::new(1, 0).id()), Fill::new([1, 2, 4]));
        assert!(!store.is_invalid());
    }

    #[test]
    fn problem_initial_store_matches_partial_store() {
        let partial = PartialPuzzle::new(4).with_given(Cell::new(1, 1), Fill::new([2]));
        let problem = partial.problem();
        assert_eq!(problem.initial_store(), &partial.store());
    }

    #[test]
    fn fixpoint_detects_contradiction() {
        // Two Given(1) cages in the same row of a 2×2 grid: all-different empties
        // a cell, so propagation reaches a contradiction and the store is invalid.
        let cell = |row, column| Polyomino::from_cells(&[Cell::new(row, column)]).unwrap();
        let partial = PartialPuzzle::new(2)
            .with_cage(CageDef::new(2, cell(0, 0), Operation::Given(1)))
            .with_cage(CageDef::new(2, cell(0, 1), Operation::Given(1)));
        assert!(fixpoint(&partial).is_invalid());
    }
}
