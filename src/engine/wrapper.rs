use std::collections::BTreeMap;

use pumpkin_solver::{
    Solver,
    conflict_resolvers::resolvers::ResolutionResolver,
    core::{
        results::{ProblemSolution, SatisfactionResult},
        termination::Indefinite,
        variables::DomainId,
    },
};

use crate::{Cell, Puzzle, types::N};

/// A complete assignment of one value to every cell, keyed by [`Cell`] in
/// row-major order.
pub type Solution = BTreeMap<Cell, N>;

/// A Pumpkin-backed CSP view of a [`Puzzle`].
///
/// Construction registers one integer decision variable per cell, seeded with
/// that cell's current candidate values. The puzzle's own constraints are not
/// yet posted to Pumpkin (that lands with the cage and Régin propagators), so
/// at this stage [`Engine::solve`] enforces only the per-cell domains.
pub struct Engine {
    solver: Solver,
    /// Cells in the order their variables were registered (row-major).
    cells: Vec<Cell>,
    /// Decision variable for each cell, parallel to `cells`.
    vars: Vec<DomainId>,
}

impl Engine {
    /// Builds an engine from a propagated puzzle, registering one sparse
    /// integer variable per cell over that cell's candidate values.
    pub fn new(puzzle: &Puzzle) -> Self {
        let mut solver = Solver::default();
        let mut cells = Vec::new();
        let mut vars = Vec::new();
        for (cell, fill) in puzzle.candidates() {
            let values: Vec<i32> = fill.iter().map(i32::from).collect();
            vars.push(solver.new_sparse_integer(values));
            cells.push(cell);
        }
        Self {
            solver,
            cells,
            vars,
        }
    }

    /// Finds one assignment consistent with every variable's domain, or `None`
    /// if the variables cannot be jointly satisfied.
    pub fn solve(&mut self) -> Option<Solution> {
        let mut brancher = self.solver.default_brancher();
        let mut termination = Indefinite;
        let mut resolver = ResolutionResolver::default();
        match self
            .solver
            .satisfy(&mut brancher, &mut termination, &mut resolver)
        {
            SatisfactionResult::Satisfiable(satisfiable) => {
                let solution = satisfiable.solution();
                Some(
                    self.cells
                        .iter()
                        .zip(&self.vars)
                        .map(|(&cell, &var)| (cell, value_of(solution.get_integer_value(var))))
                        .collect(),
                )
            }
            SatisfactionResult::Unsatisfiable(..) | SatisfactionResult::Unknown(..) => None,
        }
    }
}

/// Narrows a solved Pumpkin value to a cell value. Domains are built from
/// [`Fill`](crate::Fill), whose values lie in `1..=9`, so the conversion is
/// always in range.
fn value_of(value: i32) -> N {
    N::try_from(value).unwrap_or_else(|_| unreachable!("engine domain values lie in 1..=9"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Cage, Fill, Operation, Polyomino, constraints::test_utils::cells};

    fn given(n: u8, positions: &[(usize, usize)], value: u16) -> Cage {
        Cage::new(
            n,
            Polyomino::from_cells(&cells(positions)).unwrap(),
            Operation::Given(value),
        )
    }

    #[test]
    fn new_registers_one_variable_per_cell() {
        let puzzle = Puzzle::new(4).unwrap();
        let engine = Engine::new(&puzzle);
        assert_eq!(engine.vars.len(), 16);
        assert_eq!(engine.cells.len(), 16);
    }

    #[test]
    fn solve_assignment_lies_within_each_cell_domain() {
        // Fresh 4x4: every cell domain is {1,2,3,4}. With no constraints posted,
        // the solve still must respect the registered domains.
        let puzzle = Puzzle::new(4).unwrap();
        let domains: Vec<(Cell, Fill)> = puzzle.candidates().collect();
        let solution = Engine::new(&puzzle).solve().unwrap();
        assert_eq!(solution.len(), 16);
        for (cell, fill) in domains {
            let value = solution[&cell];
            assert!(
                !(fill & Fill::new([value])).is_empty(),
                "value {value} for {cell:?} is outside its domain"
            );
        }
    }

    #[test]
    fn solve_round_trips_a_fully_pinned_puzzle() {
        // Two Givens pin a 2x2 puzzle; the bespoke solver propagates it to all
        // singletons, so each registered domain has one value and the empty
        // solve must reproduce exactly that grid.
        let puzzle = Puzzle::with_cages(2, &[given(2, &[(0, 0)], 1), given(2, &[(0, 1)], 2)])
            .unwrap()
            .unwrap();
        let expected: Solution = puzzle
            .candidates()
            .map(|(cell, fill)| (cell, fill.iter().next().unwrap()))
            .collect();
        assert_eq!(Engine::new(&puzzle).solve().unwrap(), expected);
    }
}
