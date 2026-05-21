use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use pumpkin_solver::{
    Solver,
    conflict_resolvers::resolvers::ResolutionResolver,
    core::{
        predicate,
        predicates::Predicate,
        results::{ProblemSolution, SatisfactionResult},
        termination::Indefinite,
        variables::DomainId,
    },
};

use crate::{
    Cell, Puzzle,
    engine::{
        cage_propagator::CageArgs,
        observer_propagator::{DomainMap, DomainSink, ObserverArgs},
        regin_propagator::ReginArgs,
        value_of,
    },
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
    /// Per-cell domains at the root fixpoint, captured by the observer when the
    /// engine is built (before any search node could overwrite them).
    root_domains: DomainMap,
    /// Clause excluding the most recent solution, applied before the next
    /// search so [`Engine::next_solution`] enumerates without repeats.
    next_block: Option<Vec<Predicate>>,
    /// Set once the search space is exhausted, so further calls return `None`.
    exhausted: bool,
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

        // The observer is added last so its add-time fire — driven by
        // `add_propagator`'s propagate-to-fixpoint — sees the fully narrowed root
        // domains. Copy them out immediately; later `solve`/`enumerate` calls
        // re-fire the observer at search nodes and overwrite the shared sink.
        let sink: DomainSink = Rc::new(RefCell::new(DomainMap::new()));
        let _ = solver.add_propagator(ObserverArgs {
            cells: cells.iter().copied().collect(),
            vars: vars.iter().copied().collect(),
            sink: Rc::clone(&sink),
        });
        let root_domains = sink.borrow().clone();

        Self {
            solver,
            cells,
            vars,
            root_domains,
            next_block: None,
            exhausted: false,
        }
    }

    /// Returns each cell's candidate domain at the root propagation fixpoint —
    /// the per-cell display the Designer needs without running a full solve.
    pub fn propagate_to_fixpoint(&self) -> DomainMap {
        self.root_domains.clone()
    }

    /// Lazily returns the next distinct solution, or `None` once the search is
    /// exhausted (or proven unsatisfiable).
    ///
    /// Each call first excludes the previous solution with a blocking clause —
    /// the same scheme Pumpkin's solution iterator uses — so repeated calls
    /// enumerate without repeats. Driving it call-by-call keeps `Puzzle::solve`
    /// lazy: a uniqueness check can stop after two solutions without computing
    /// the rest.
    pub fn next_solution(&mut self) -> Option<Solution> {
        if self.exhausted {
            return None;
        }
        if let Some(block) = self.next_block.take() {
            let tag = self.solver.new_constraint_tag();
            if self.solver.add_clause(block, tag).is_err() {
                self.exhausted = true;
                return None;
            }
        }
        let mut brancher = self.solver.default_brancher();
        let mut termination = Indefinite;
        let mut resolver = ResolutionResolver::default();
        let found = match self
            .solver
            .satisfy(&mut brancher, &mut termination, &mut resolver)
        {
            SatisfactionResult::Satisfiable(satisfiable) => {
                let solution = satisfiable.solution();
                let assignment = read(&self.cells, &self.vars, &solution);
                let block = self
                    .vars
                    .iter()
                    .map(|&var| {
                        let value = solution.get_integer_value(var);
                        predicate![var != value]
                    })
                    .collect();
                Some((assignment, block))
            }
            SatisfactionResult::Unsatisfiable(..) | SatisfactionResult::Unknown(..) => None,
        };
        if let Some((assignment, block)) = found {
            self.next_block = Some(block);
            Some(assignment)
        } else {
            self.exhausted = true;
            None
        }
    }

    /// Enumerates up to `limit` distinct solutions.
    ///
    /// Used for uniqueness checks: stopping after two solutions distinguishes a
    /// uniquely-solvable puzzle from an ambiguous one.
    pub fn enumerate(&mut self, limit: usize) -> Vec<Solution> {
        let mut solutions = Vec::new();
        while solutions.len() < limit {
            match self.next_solution() {
                Some(solution) => solutions.push(solution),
                None => break,
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
        let solution = Engine::new(&puzzle).next_solution().unwrap();
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
        let solution = Engine::new(&puzzle).next_solution().unwrap();
        assert_eq!(solution[&Cell::new(0, 0)], 3);
    }

    #[test]
    fn solve_respects_arithmetic_cage() {
        // A 2-cell Subtract(1) cage in column 0: the two cells differ by 1.
        let puzzle = Puzzle::with_cages(4, &[cage(4, &[(0, 0), (1, 0)], Operation::Subtract(1))])
            .unwrap()
            .unwrap();
        let solution = Engine::new(&puzzle).next_solution().unwrap();
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

    /// The bespoke solver's fixpoint domains as a `Cell -> Fill` map.
    fn reference_domains(puzzle: &Puzzle) -> DomainMap {
        puzzle.candidates().collect()
    }

    #[test]
    fn fixpoint_domains_match_bespoke_solver_on_empty_grid() {
        let puzzle = Puzzle::new(4).unwrap();
        assert_eq!(
            Engine::new(&puzzle).propagate_to_fixpoint(),
            reference_domains(&puzzle)
        );
    }

    #[test]
    fn fixpoint_domains_match_bespoke_solver_with_given_cage() {
        // Given(3) at (0,0) pins that cell and narrows its row and column.
        let puzzle = Puzzle::with_cages(4, &[cage(4, &[(0, 0)], Operation::Given(3))])
            .unwrap()
            .unwrap();
        let domains = Engine::new(&puzzle).propagate_to_fixpoint();
        assert_eq!(domains, reference_domains(&puzzle));
        assert_eq!(domains[&Cell::new(0, 0)], crate::Fill::new([3]));
    }

    #[test]
    fn fixpoint_domains_match_bespoke_solver_on_partially_filled_puzzles() {
        // A battery of partially-filled puzzles: the engine's fixpoint per-cell
        // domains must match the bespoke solver's exactly.
        let puzzles = [
            Puzzle::with_cages(
                4,
                &[
                    cage(4, &[(0, 0), (0, 1)], Operation::Subtract(1)),
                    cage(4, &[(1, 0)], Operation::Given(2)),
                ],
            )
            .unwrap()
            .unwrap(),
            Puzzle::with_cages(
                4,
                &[
                    cage(4, &[(0, 0), (1, 0)], Operation::Divide(2)),
                    cage(4, &[(0, 3)], Operation::Given(4)),
                ],
            )
            .unwrap()
            .unwrap(),
            Puzzle::with_cages(
                3,
                &[
                    cage(3, &[(0, 0), (0, 1), (0, 2)], Operation::Add(6)),
                    cage(3, &[(1, 0)], Operation::Given(1)),
                ],
            )
            .unwrap()
            .unwrap(),
        ];
        for puzzle in &puzzles {
            assert_eq!(
                Engine::new(puzzle).propagate_to_fixpoint(),
                reference_domains(puzzle),
                "fixpoint mismatch for puzzle {puzzle:?}"
            );
        }
    }
}
