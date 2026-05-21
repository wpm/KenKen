use std::{collections::BTreeMap, rc::Rc};

use pumpkin_solver::{
    Solver,
    conflict_resolvers::resolvers::ResolutionResolver,
    core::{
        results::{ProblemSolution, SatisfactionResult, solution_iterator::IteratedSolution},
        termination::Indefinite,
        variables::DomainId,
    },
};

use crate::{
    Cell, Puzzle,
    engine::{cage_propagator::CageArgs, regin_propagator::ReginArgs, value_of},
    types::N,
};

/// A complete assignment of one value to every cell, keyed by [`Cell`] in
/// row-major order.
pub type Solution = BTreeMap<Cell, N>;

/// A Pumpkin-backed CSP view of a [`Puzzle`].
///
/// Construction registers one integer decision variable per cell over `1..=n`,
/// posts a generalized-Régin all-different
/// ([`ReginPropagator`](super::regin_propagator::ReginPropagator)) for every row
/// and column, and adds a
/// [`CagePropagator`](super::cage_propagator::CagePropagator) for each cage.
pub struct Engine {
    solver: Solver,
    /// Cells in row-major order; `vars[i]` is the variable for `cells[i]`.
    cells: Vec<Cell>,
    /// Decision variable per cell, indexed in row-major order.
    vars: Vec<DomainId>,
}

impl Engine {
    /// Builds an engine modelling `puzzle`: a variable per cell, row/column
    /// all-different, and a cage propagator per cage.
    pub fn new(puzzle: &Puzzle) -> Self {
        let n = puzzle.n();
        let mut solver = Solver::default();
        let cells: Vec<Cell> = (0..n)
            .flat_map(|row| (0..n).map(move |column| Cell::new(row, column)))
            .collect();
        let upper = i32::try_from(n).unwrap_or_else(|_| unreachable!("grid size n lies in 1..=9"));
        let vars: Vec<DomainId> = cells
            .iter()
            .map(|_| solver.new_bounded_integer(1, upper))
            .collect();
        let var_at = |cell: Cell| vars[cell.row * n + cell.column];
        let grid_size =
            N::try_from(n).unwrap_or_else(|_| unreachable!("grid size n lies in 1..=9"));

        // Posting cannot fail at the root for a valid puzzle: all-different over
        // distinct variables is root-consistent, and a stored cage always has at
        // least one tuple (an empty one would have failed puzzle construction),
        // so its propagator empties no domain from full domains.
        for index in 0..n {
            for line in [row_cells(n, index), column_cells(n, index)] {
                let line_vars: Rc<[DomainId]> = line.into_iter().map(var_at).collect();
                let tag = solver.new_constraint_tag();
                let _ = solver.add_propagator(ReginArgs {
                    vars: line_vars,
                    n: grid_size,
                    tag,
                });
            }
        }

        for cage in puzzle.cages() {
            let cage_vars: Rc<[DomainId]> = cage.cells().map(var_at).collect();
            let tuples: Rc<[Vec<N>]> = cage.tuples().iter().cloned().collect();
            let tag = solver.new_constraint_tag();
            let _ = solver.add_propagator(CageArgs {
                vars: cage_vars,
                tuples,
                tag,
            });
        }

        Self {
            solver,
            cells,
            vars,
        }
    }

    /// Finds one solution to the puzzle, or `None` if it is unsatisfiable.
    pub fn solve(&mut self) -> Option<Solution> {
        let mut brancher = self.solver.default_brancher();
        let mut termination = Indefinite;
        let mut resolver = ResolutionResolver::default();
        match self
            .solver
            .satisfy(&mut brancher, &mut termination, &mut resolver)
        {
            SatisfactionResult::Satisfiable(satisfiable) => {
                Some(read(&self.cells, &self.vars, &satisfiable.solution()))
            }
            SatisfactionResult::Unsatisfiable(..) | SatisfactionResult::Unknown(..) => None,
        }
    }

    /// Enumerates up to `limit` distinct solutions.
    ///
    /// Used for uniqueness checks: stopping after two solutions distinguishes a
    /// uniquely-solvable puzzle from an ambiguous one. Enumeration adds blocking
    /// clauses to the solver, so an `Engine` should be enumerated only once.
    pub fn enumerate(&mut self, limit: usize) -> Vec<Solution> {
        if limit == 0 {
            return vec![];
        }
        // Snapshot the cell/variable pairing so the read loop borrows neither
        // `self` nor the solver that the solution iterator holds mutably.
        let pairs: Vec<(Cell, DomainId)> = self
            .cells
            .iter()
            .copied()
            .zip(self.vars.iter().copied())
            .collect();
        let mut brancher = self.solver.default_brancher();
        let mut termination = Indefinite;
        let mut resolver = ResolutionResolver::default();
        let mut iterator =
            self.solver
                .get_solution_iterator(&mut brancher, &mut termination, &mut resolver);
        let mut solutions = Vec::new();
        while solutions.len() < limit {
            match iterator.next_solution() {
                IteratedSolution::Solution(solution, ..) => {
                    solutions.push(
                        pairs
                            .iter()
                            .map(|&(cell, var)| (cell, value_of(solution.get_integer_value(var))))
                            .collect(),
                    );
                }
                IteratedSolution::Finished
                | IteratedSolution::Unknown
                | IteratedSolution::Unsatisfiable => break,
            }
        }
        solutions
    }
}

/// Reads each cell's assigned value out of a Pumpkin solution.
fn read(cells: &[Cell], vars: &[DomainId], solution: &impl ProblemSolution) -> Solution {
    cells
        .iter()
        .zip(vars)
        .map(|(&cell, &var)| (cell, value_of(solution.get_integer_value(var))))
        .collect()
}

/// The cells of row `index` on an `n`×`n` grid, left to right.
fn row_cells(n: usize, index: usize) -> Vec<Cell> {
    (0..n).map(|column| Cell::new(index, column)).collect()
}

/// The cells of column `index` on an `n`×`n` grid, top to bottom.
fn column_cells(n: usize, index: usize) -> Vec<Cell> {
    (0..n).map(|row| Cell::new(row, index)).collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Cage, Operation, Polyomino, constraints::test_utils::cells};

    fn cage(n: u8, positions: &[(usize, usize)], op: Operation) -> Cage {
        Cage::new(n, Polyomino::from_cells(&cells(positions)).unwrap(), op)
    }

    /// Solutions the bespoke solver finds, as `Cell -> value` maps, for
    /// cross-checking the engine.
    fn reference_solutions(puzzle: &Puzzle) -> Vec<Solution> {
        puzzle
            .solve()
            .map(|solved| {
                solved
                    .candidates()
                    .map(|(cell, fill)| (cell, fill.iter().next().unwrap()))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn solve_satisfies_row_and_column_all_different() {
        let puzzle = Puzzle::new(4).unwrap();
        let solution = Engine::new(&puzzle).solve().unwrap();
        for index in 0..4 {
            let row: Vec<N> = (0..4).map(|c| solution[&Cell::new(index, c)]).collect();
            let col: Vec<N> = (0..4).map(|r| solution[&Cell::new(r, index)]).collect();
            for line in [row, col] {
                let mut sorted = line.clone();
                sorted.sort_unstable();
                sorted.dedup();
                assert_eq!(sorted.len(), line.len(), "line has a repeat: {line:?}");
            }
        }
    }

    #[test]
    fn solve_respects_given_cage() {
        let puzzle = Puzzle::with_cages(4, &[cage(4, &[(0, 0)], Operation::Given(3))])
            .unwrap()
            .unwrap();
        let solution = Engine::new(&puzzle).solve().unwrap();
        assert_eq!(solution[&Cell::new(0, 0)], 3);
    }

    #[test]
    fn solve_respects_arithmetic_cage() {
        // A 2-cell Subtract(1) cage in column 0: the two cells differ by 1.
        let puzzle = Puzzle::with_cages(4, &[cage(4, &[(0, 0), (1, 0)], Operation::Subtract(1))])
            .unwrap()
            .unwrap();
        let solution = Engine::new(&puzzle).solve().unwrap();
        let a = i32::from(solution[&Cell::new(0, 0)]);
        let b = i32::from(solution[&Cell::new(1, 0)]);
        assert_eq!((a - b).abs(), 1);
    }

    #[test]
    fn enumerate_agrees_with_bespoke_solver_on_unique_puzzle() {
        // Givens on the whole top row + left column of a 3x3 pin a unique grid.
        let puzzle = Puzzle::with_cages(
            3,
            &[
                cage(3, &[(0, 0)], Operation::Given(1)),
                cage(3, &[(0, 1)], Operation::Given(2)),
                cage(3, &[(0, 2)], Operation::Given(3)),
                cage(3, &[(1, 0)], Operation::Given(2)),
                cage(3, &[(2, 0)], Operation::Given(3)),
            ],
        )
        .unwrap()
        .unwrap();
        let reference = reference_solutions(&puzzle);
        assert_eq!(reference.len(), 1);
        let found = Engine::new(&puzzle).enumerate(10);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], reference[0]);
    }

    #[test]
    fn enumerate_count_matches_bespoke_solver_on_open_puzzle() {
        // A 3x3 Latin square with no cages has 12 completions.
        let puzzle = Puzzle::new(3).unwrap();
        let reference = reference_solutions(&puzzle);
        let found = Engine::new(&puzzle).enumerate(usize::MAX);
        assert_eq!(found.len(), reference.len());
        assert_eq!(found.len(), 12);
    }

    #[test]
    fn enumerate_respects_limit() {
        let puzzle = Puzzle::new(4).unwrap();
        assert_eq!(Engine::new(&puzzle).enumerate(5).len(), 5);
    }

    #[test]
    fn enumerate_zero_limit_is_empty() {
        let puzzle = Puzzle::new(4).unwrap();
        assert!(Engine::new(&puzzle).enumerate(0).is_empty());
    }
}
