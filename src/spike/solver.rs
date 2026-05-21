//! The CS-nomenclature [`Solver`] trait and a plain DFS [`BacktrackSolver`].

use std::marker::PhantomData;

use crate::{
    Cell, Fill,
    spike::{
        KenKenConstraint,
        cache::Cache,
        constraint::{Constraint, Outcome, PropagationCtx, propagate_to_fixpoint},
        problem::Problem,
        store::Store,
        variable::Variable,
    },
};

/// Enumerates the solutions to a [`Problem`].
pub trait Solver<V: Variable, C: Constraint<V>> {
    fn solve(&mut self, problem: &Problem<V, C>) -> SolveResult<V>;
}

/// The solved stores found by a [`Solver`].
pub struct SolveResult<V: Variable> {
    solutions: Vec<Store>,
    marker: PhantomData<V>,
}

impl<V: Variable> SolveResult<V> {
    fn new(solutions: Vec<Store>) -> Self {
        Self {
            solutions,
            marker: PhantomData,
        }
    }

    pub fn solutions(&self) -> &[Store] {
        &self.solutions
    }
}

/// Plain depth-first backtracking: propagate to a fixed point at each node,
/// prune on contradiction, branch on the most-constrained unsolved cell. No
/// restarts, heuristics, or conflict learning — just enough to solve the test
/// instance. The cache is shared across the whole search (global memoization of
/// viable tuples).
#[derive(Default)]
pub struct BacktrackSolver {
    cache: Cache,
}

impl BacktrackSolver {
    pub fn new() -> Self {
        Self {
            cache: Cache::default(),
        }
    }
}

impl Solver<Cell, KenKenConstraint> for BacktrackSolver {
    fn solve(&mut self, problem: &Problem<Cell, KenKenConstraint>) -> SolveResult<Cell> {
        let mut solutions: Vec<Store> = Vec::new();
        let mut stack: Vec<Store> = vec![problem.initial_store().clone()];
        while let Some(mut store) = stack.pop() {
            let outcome = {
                let mut ctx = PropagationCtx::new(&mut store, &mut self.cache);
                propagate_to_fixpoint(&mut ctx, problem.constraints())
            };
            if outcome == Outcome::Contradiction || store.is_invalid() {
                continue;
            }
            // After the validity check, an all-singleton store is a solution and
            // has no cell to branch on, so `None` from `most_constrained` is
            // exactly "solved".
            match most_constrained(&store) {
                None => solutions.push(store),
                Some(cell) => {
                    for value in store.get(cell.id()).iter() {
                        let mut child = store.clone();
                        child.set(cell.id(), Fill::new([value]));
                        stack.push(child);
                    }
                }
            }
        }
        SolveResult::new(solutions)
    }
}

/// The unsolved cell with the fewest candidates, breaking ties by row-major
/// cell order. `None` when every cell is a singleton.
fn most_constrained(store: &Store) -> Option<Cell> {
    store
        .cells()
        .map(|cell| (cell, store.get(cell.id())))
        .filter(|(_, domain)| domain.len() > 1)
        .min_by(|(a, da), (b, db)| da.len().cmp(&db.len()).then_with(|| a.cmp(b)))
        .map(|(cell, _)| cell)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{
        Operation, Polyomino,
        spike::{cage::CageDef, problem::PartialPuzzle, variable::Variable},
    };

    fn singleton(row: usize, column: usize, value: crate::types::M) -> CageDef {
        let poly = Polyomino::from_cells(&[Cell::new(row, column)]).unwrap();
        CageDef::new(2, poly, Operation::Given(value))
    }

    #[test]
    fn solves_a_tiny_fully_pinned_puzzle() {
        // 2×2 with three givens forces a unique solution.
        let partial = PartialPuzzle::new(2)
            .with_cage(singleton(0, 0, 1))
            .with_cage(singleton(0, 1, 2))
            .with_cage(singleton(1, 0, 2));
        let result = BacktrackSolver::new().solve(&partial.problem());
        assert_eq!(result.solutions().len(), 1);
        let store = &result.solutions()[0];
        assert_eq!(store.get(Cell::new(1, 1).id()), Fill::new([1]));
    }

    #[test]
    fn empty_2x2_has_two_solutions() {
        // No cages: a 2×2 Latin square has exactly two solutions.
        let result = BacktrackSolver::new().solve(&PartialPuzzle::new(2).problem());
        assert_eq!(result.solutions().len(), 2);
    }

    #[test]
    fn empty_3x3_has_twelve_solutions() {
        // No cages: there are exactly twelve 3×3 Latin squares.
        let result = BacktrackSolver::new().solve(&PartialPuzzle::new(3).problem());
        assert_eq!(result.solutions().len(), 12);
    }

    #[test]
    fn empty_4x4_has_576_solutions() {
        // No cages: there are exactly 576 4×4 Latin squares. Enumerating them
        // forces the solver to branch deeply and to prune contradictory branches.
        let result = BacktrackSolver::new().solve(&PartialPuzzle::new(4).problem());
        assert_eq!(result.solutions().len(), 576);
    }

    #[test]
    fn most_constrained_is_none_for_solved_store() {
        let mut store = Store::full(2);
        for row in 0..2 {
            for column in 0..2 {
                store.set(Cell::new(row, column).id(), Fill::new([1]));
            }
        }
        assert!(most_constrained(&store).is_none());
    }

    #[test]
    fn most_constrained_picks_fewest_candidates() {
        let mut store = Store::full(4);
        store.set(Cell::new(0, 0).id(), Fill::new([1, 2, 3]));
        store.set(Cell::new(2, 2).id(), Fill::new([1, 2]));
        assert_eq!(most_constrained(&store), Some(Cell::new(2, 2)));
    }

    #[test]
    fn default_solver_matches_new() {
        let result = BacktrackSolver::default().solve(&PartialPuzzle::new(2).problem());
        assert_eq!(result.solutions().len(), 2);
    }
}
