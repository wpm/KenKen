//! `Puzzle` is `Send + Sync` and may be shared across threads or sent
//! between them. Cloning is cheap (the constraints share an `Arc`).

#![allow(dead_code)]
use crate::constraints::{AllDifferent, Cage, Cages, PuzzleConstraints};
use crate::delta::Delta;
use crate::grid::Grid;
use crate::shape::Polyomino;
use crate::solver::{Solver, State};
use crate::types::{Cell, Error, Index, N, Values};
use std::sync::Arc;

/// Three-bucket classification of a puzzle's solution count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Uniqueness {
    /// No solutions.
    None,
    /// Exactly one solution.
    Unique,
    /// Two or more solutions.
    Multiple,
}

/// A KenKen puzzle: a candidate-value grid paired with a fixed set of all-different and
/// cage constraints.
///
/// Construct one via [`crate::generate`] or [`crate::generate_with`]; the public surface is
/// [`Puzzle::n`], [`Puzzle::uniqueness`], and [`Puzzle::solutions`].
///
/// ## Cloning during search
///
/// Solving requires branching — each branch needs its own copy of the puzzle state.
/// `Puzzle` is designed so that clone is cheap:
///
/// - The **grid** (candidate bitmaps) is a flat boxed slice copied in a single `memcpy`.
/// - The **constraints** (rows, columns, cages) never change after construction, so they are
///   stored behind an [`Arc`]. Cloning bumps a reference count rather than duplicating data.
///   Mutating methods use [`Arc::make_mut`] to copy-on-write only when necessary.
#[must_use]
#[derive(Debug, Clone)]
pub struct Puzzle {
    grid: Grid,
    constraints: Arc<PuzzleConstraints>,
}

impl Puzzle {
    /// # Errors
    /// Returns `Error` if `n` is not in `1..=9`.
    pub fn new(n: Index) -> Result<Self, Error> {
        Ok(Self {
            grid: Grid::new(n)?,
            constraints: Arc::new(PuzzleConstraints {
                row: (0..n).map(|row| AllDifferent::row(n, row)).collect(),
                column: (0..n)
                    .map(|column| AllDifferent::column(n, column))
                    .collect(),
                cage: Cages::empty(n),
            }),
        })
    }

    /// The side length of the grid (number of rows and columns).
    #[must_use]
    pub const fn n(&self) -> Index {
        self.grid.n()
    }

    /// Returns a new puzzle with the cage inserted.
    ///
    /// Idempotent: if the exact same cage (by polyomino) is already present, returns unchanged.
    /// # Errors
    /// Returns `Error` if any cell in the cage is already claimed by a *different* cage.
    pub fn insert_cage(mut self, cage: Cage) -> Result<Self, Error> {
        let constraints = Arc::make_mut(&mut self.constraints);
        constraints.cage = constraints.cage.clone().insert(cage)?;
        Ok(self)
    }

    /// Returns a new puzzle with the cage removed.
    ///
    /// Idempotent: if no such cage exists, returns unchanged.
    pub fn remove_cage(mut self, polyomino: &Polyomino) -> Self {
        let constraints = Arc::make_mut(&mut self.constraints);
        constraints.cage = constraints.cage.clone().remove(polyomino);
        self
    }

    /// Classifies the puzzle's solution count into [`Uniqueness::None`], [`Uniqueness::Unique`],
    /// or [`Uniqueness::Multiple`].
    ///
    /// Stops the solver as soon as a second solution is found, so this is strictly cheaper than
    /// [`Puzzle::solutions`] when the answer is `Multiple`.
    #[must_use]
    pub fn uniqueness(&self) -> Uniqueness {
        match self.solutions_at_most(2) {
            0 => Uniqueness::None,
            1 => Uniqueness::Unique,
            _ => Uniqueness::Multiple,
        }
    }

    /// Counts every solution by exhaustive search.
    ///
    /// Use [`Puzzle::uniqueness`] when only the bucket (none / one / many) is needed.
    #[must_use]
    pub fn solutions(&self) -> usize {
        self.solutions_at_most(usize::MAX)
    }

    /// Count solutions, stopping after `k` are found.
    /// Returns `min(actual_count, k)`.
    ///
    /// `solutions_at_most(0)` returns `0` without invoking the solver.
    /// `solutions_at_most(usize::MAX)` is equivalent to [`Puzzle::solutions`].
    #[must_use]
    pub fn solutions_at_most(&self, k: usize) -> usize {
        if k == 0 {
            return 0;
        }
        Solver::new(self.clone()).take(k).count()
    }

    /// Read-only access to the underlying candidate-value grid.
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Returns the candidate values currently associated with `cell`.
    /// # Errors
    /// Returns `Error` if `cell` is outside the grid bounds.
    pub fn candidates(&self, cell: Cell) -> Result<Values, Error> {
        self.grid.get(&cell)
    }

    /// Iterates over every cell of the grid in row-major order.
    pub fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        self.grid.iter()
    }

    /// Iterates over cells whose candidate set is a singleton, paired with that value.
    pub fn singleton_cells(&self) -> impl Iterator<Item = (Cell, N)> + '_ {
        self.grid.iter_with_values().filter_map(|(cell, v)| {
            if !v.is_singleton() {
                return None;
            }
            v.iter().next().map(|n| (cell, n))
        })
    }

    /// Iterates over cells whose candidate set is empty.
    pub fn empty_cells(&self) -> impl Iterator<Item = Cell> + '_ {
        self.grid
            .iter_with_values()
            .filter_map(|(cell, v)| v.is_empty().then_some(cell))
    }

    /// Returns the cage covering `cell`, or `None` if none does.
    #[must_use]
    pub fn cage_at(&self, cell: Cell) -> Option<&Cage> {
        self.constraints.cage.get_at(cell)
    }

    /// Iterates over every cage in the puzzle.
    pub fn cages(&self) -> impl Iterator<Item = &Cage> + '_ {
        self.constraints.cage.iter()
    }

    /// Returns true iff some cage covers `cell`.
    #[must_use]
    pub fn is_covered(&self, cell: Cell) -> bool {
        self.cage_at(cell).is_some()
    }

    /// Returns true iff every cell has at least one candidate value — i.e. propagation
    /// has not produced a contradiction.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.grid.is_invalid()
    }

    /// One round of constraint propagation: builds a value filter from every constraint
    /// against the current grid and applies it. Always returns a `Puzzle`; the resulting
    /// grid may be invalid when constraints contradict. Best-effort: returns `self`
    /// unchanged on the unreachable error path from `apply`.
    fn propagate_round(self) -> Self {
        let Ok(filter) = self.constraints.apply(&self.grid) else {
            return self;
        };
        let Ok(grid) = filter.apply(&self.grid) else {
            return self;
        };
        Self {
            grid,
            constraints: self.constraints,
        }
    }

    /// Iterates one round of constraint propagation until the grid stops changing or
    /// goes invalid. Always returns a `Puzzle`; the caller inspects [`Self::is_valid`]
    /// if they care.
    pub fn propagate_fully(self) -> Self {
        let mut current = self;
        loop {
            let prev_grid = current.grid.clone();
            let next = current.propagate_round();
            if next.grid.is_invalid() || next.grid == prev_grid {
                return next;
            }
            current = next;
        }
    }

    /// Apply `delta` by intersecting per-cell with the current grid, then propagate to
    /// fixed point. Always returns a `Puzzle`; the caller inspects [`Self::is_valid`].
    ///
    /// # Panics
    /// Panics if `delta.n()` does not match `self.n()`.
    pub fn narrow(&self, delta: &Delta) -> Self {
        self.apply_delta(delta, |a, b| a & b)
    }

    /// Apply `delta` by unioning per-cell with the current grid, then propagate to
    /// fixed point. Always returns a `Puzzle`; the caller inspects [`Self::is_valid`].
    ///
    /// # Panics
    /// Panics if `delta.n()` does not match `self.n()`.
    pub fn widen(&self, delta: &Delta) -> Self {
        self.apply_delta(delta, |a, b| a | b)
    }

    fn apply_delta(&self, delta: &Delta, op: impl Fn(Values, Values) -> Values) -> Self {
        assert_eq!(
            delta.n(),
            self.n(),
            "delta size {} must match puzzle size {}",
            delta.n(),
            self.n(),
        );
        let mut grid = self.grid.clone();
        for ((cell, current), (_, delta_v)) in self
            .grid
            .iter_with_values()
            .zip(delta.grid().iter_with_values())
        {
            grid = grid.set(&cell, op(current, delta_v));
        }
        Self {
            grid,
            constraints: self.constraints.clone(),
        }
        .propagate_fully()
    }
}

impl State for Puzzle {
    fn propagate(self) -> Option<Self> {
        let next = self.propagate_round();
        if next.grid.is_invalid() {
            None
        } else {
            Some(next)
        }
    }

    fn branch(self) -> impl Iterator<Item = Self> {
        let pivot = self
            .grid
            .iter()
            .filter_map(|cell| {
                let values = self.grid.get(&cell).ok()?;
                (!values.is_singleton()).then_some((cell, values))
            })
            .min_by_key(|(cell, values)| (values.len(), *cell));
        let Some((cell, values)) = pivot else {
            return itertools::Either::Left(std::iter::empty());
        };
        let constraints = self.constraints.clone();
        let grid = self.grid;
        itertools::Either::Right(values.iter().map(move |v| Self {
            grid: grid.clone().set(&cell, Values::new([v])),
            constraints: constraints.clone(),
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::constraints::Operation;
    use crate::types::Cell;

    fn make_cage(cells: &[(usize, usize)], n: u8) -> Cage {
        let cells: Vec<Cell> = cells
            .iter()
            .map(|&(row, column)| Cell::new(row, column))
            .collect();
        Cage::new(n, Polyomino::new(&cells), Operation::Add(n))
    }

    fn poly(cells: &[(usize, usize)]) -> Polyomino {
        Polyomino::new(
            &cells
                .iter()
                .map(|&(row, column)| Cell::new(row, column))
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn new_returns_err_for_invalid_size() {
        assert!(Puzzle::new(0).is_err());
        assert!(Puzzle::new(10).is_err());
    }

    #[test]
    fn new_n_matches_size() {
        assert_eq!(Puzzle::new(4).unwrap().n(), 4);
    }

    #[test]
    fn insert_cage_adds_cage() {
        let cage = make_cage(&[(0, 0), (0, 1)], 4);
        let poly = cage.polyomino().clone();
        let puzzle = Puzzle::new(4).unwrap().insert_cage(cage).unwrap();
        assert!(puzzle.constraints.cage.contains(&poly));
    }

    #[test]
    fn insert_cage_cage_covers_correct_cells() {
        let cage = make_cage(&[(0, 0), (0, 1)], 4);
        let poly = cage.polyomino().clone();
        let puzzle = Puzzle::new(4).unwrap().insert_cage(cage).unwrap();
        let cells = puzzle.constraints.cage.get(&poly).unwrap().cells();
        assert!(cells.contains(&Cell::new(0, 0)));
        assert!(cells.contains(&Cell::new(0, 1)));
    }

    #[test]
    fn insert_two_non_overlapping_cages() {
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(make_cage(&[(0, 0), (0, 1)], 4))
            .unwrap()
            .insert_cage(make_cage(&[(1, 0), (1, 1)], 4))
            .unwrap();
        assert_eq!(puzzle.constraints.cage.len(), 2);
    }

    #[test]
    fn insert_cage_idempotent() {
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(make_cage(&[(0, 0), (0, 1)], 4))
            .unwrap()
            .insert_cage(make_cage(&[(0, 0), (0, 1)], 4))
            .unwrap();
        assert_eq!(puzzle.constraints.cage.len(), 1);
    }

    #[test]
    fn insert_cage_conflict_returns_err() {
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(make_cage(&[(0, 0), (0, 1)], 4))
            .unwrap();
        let result = puzzle.insert_cage(make_cage(&[(0, 1), (0, 2)], 4));
        assert!(matches!(result, Err(Error::CageConflict(_, _))));
    }

    #[test]
    fn insert_cage_conflict_carries_new_and_existing_cage() {
        let existing = make_cage(&[(0, 0), (0, 1)], 4);
        let new = make_cage(&[(0, 1), (0, 2)], 4);
        let existing_poly = existing.polyomino().clone();
        let new_poly = new.polyomino().clone();
        let puzzle = Puzzle::new(4).unwrap().insert_cage(existing).unwrap();
        if let Err(Error::CageConflict(got_new, got_existing)) = puzzle.insert_cage(new) {
            assert_eq!(got_new.polyomino(), &new_poly);
            assert_eq!(got_existing.polyomino(), &existing_poly);
        } else {
            unreachable!("expected CageConflict");
        }
    }

    #[test]
    fn remove_cage_removes_cage() {
        let cage = make_cage(&[(0, 0), (0, 1)], 4);
        let poly = cage.polyomino().clone();
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(cage)
            .unwrap()
            .remove_cage(&poly);
        assert!(!puzzle.constraints.cage.contains(&poly));
    }

    #[test]
    fn remove_cage_leaves_other_cages_intact() {
        let a = make_cage(&[(0, 0), (0, 1)], 4);
        let b = make_cage(&[(1, 0), (1, 1)], 4);
        let poly_a = a.polyomino().clone();
        let poly_b = b.polyomino().clone();
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(a)
            .unwrap()
            .insert_cage(b)
            .unwrap()
            .remove_cage(&poly_a);
        assert!(!puzzle.constraints.cage.contains(&poly_a));
        assert!(puzzle.constraints.cage.contains(&poly_b));
    }

    #[test]
    fn remove_cage_idempotent() {
        let p = poly(&[(0, 0), (0, 1)]);
        let _ = Puzzle::new(4).unwrap().remove_cage(&p).remove_cage(&p);
    }

    #[test]
    fn remove_then_insert_succeeds() {
        let cage = make_cage(&[(0, 0), (0, 1)], 4);
        let poly = cage.polyomino().clone();
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(cage)
            .unwrap()
            .remove_cage(&poly)
            .insert_cage(make_cage(&[(0, 0), (0, 1)], 4))
            .unwrap();
        assert!(puzzle.constraints.cage.contains(&poly));
    }

    #[test]
    fn clone_shares_constraints() {
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(make_cage(&[(0, 0), (0, 1)], 4))
            .unwrap();
        let clone = puzzle.clone();
        assert!(Arc::ptr_eq(&puzzle.constraints, &clone.constraints));
    }

    fn solve(puzzle: Puzzle) -> Vec<Vec<Vec<u8>>> {
        Solver::new(puzzle)
            .map(|p| {
                let n = p.n();
                (0..n)
                    .map(|row| {
                        (0..n)
                            .map(|col| {
                                p.grid
                                    .get(&Cell::new(row, col))
                                    .unwrap()
                                    .iter()
                                    .next()
                                    .unwrap()
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect()
    }

    fn given(row: usize, column: usize, value: u8) -> Cage {
        let cell = Cell::new(row, column);
        Cage::new(value, Polyomino::new(&[cell]), Operation::Given(value))
    }

    fn row_add(n: u8, row: usize, target: u8) -> Cage {
        let cells: Vec<Cell> = (0..n as usize).map(|col| Cell::new(row, col)).collect();
        Cage::new(n, Polyomino::new(&cells), Operation::Add(target))
    }

    #[test]
    fn solve_1x1_singleton_solves_immediately() {
        // 1×1 grid: the single cell is already a singleton, solved without branching.
        let solutions = solve(Puzzle::new(1).unwrap());
        assert_eq!(solutions, vec![vec![vec![1u8]]]);
    }

    #[test]
    fn solve_2x2_no_solution() {
        // Given(1) in both cells of row 0 conflicts with all-different: no valid assignment.
        let puzzle = Puzzle::new(2)
            .unwrap()
            .insert_cage(given(0, 0, 1))
            .unwrap()
            .insert_cage(given(0, 1, 1))
            .unwrap();
        assert_eq!(solve(puzzle), Vec::<Vec<Vec<u8>>>::new());
    }

    #[test]
    fn solve_2x2_one_solution() {
        // Given(1) at (0,0): all-different forces (0,1)=2, (1,0)=2, (1,1)=1.
        let puzzle = Puzzle::new(2).unwrap().insert_cage(given(0, 0, 1)).unwrap();
        assert_eq!(solve(puzzle), vec![vec![vec![1, 2], vec![2, 1]]]);
    }

    #[test]
    fn solve_2x2_multiple_solutions() {
        // Row add-cages both requiring sum=3 admit both 2×2 latin squares.
        // DFS with LIFO stack pops the highest-value branch first, so [[2,1],[1,2]] is found first.
        let puzzle = Puzzle::new(2)
            .unwrap()
            .insert_cage(row_add(2, 0, 3))
            .unwrap()
            .insert_cage(row_add(2, 1, 3))
            .unwrap();
        assert_eq!(
            solve(puzzle),
            vec![vec![vec![2, 1], vec![1, 2]], vec![vec![1, 2], vec![2, 1]],]
        );
    }

    #[test]
    fn solutions_at_most_zero_returns_zero() {
        let unique = Puzzle::new(2).unwrap().insert_cage(given(0, 0, 1)).unwrap();
        let multi = Puzzle::new(2)
            .unwrap()
            .insert_cage(row_add(2, 0, 3))
            .unwrap()
            .insert_cage(row_add(2, 1, 3))
            .unwrap();
        let none = Puzzle::new(2)
            .unwrap()
            .insert_cage(given(0, 0, 1))
            .unwrap()
            .insert_cage(given(0, 1, 1))
            .unwrap();
        assert_eq!(unique.solutions_at_most(0), 0);
        assert_eq!(multi.solutions_at_most(0), 0);
        assert_eq!(none.solutions_at_most(0), 0);
    }

    #[test]
    fn solutions_at_most_caps_unique_puzzle() {
        let p = Puzzle::new(2).unwrap().insert_cage(given(0, 0, 1)).unwrap();
        assert_eq!(p.solutions_at_most(1), 1);
        assert_eq!(p.solutions_at_most(2), 1);
        assert_eq!(p.solutions_at_most(100), 1);
    }

    #[test]
    fn solutions_at_most_caps_multi_solution_puzzle() {
        let p = Puzzle::new(2)
            .unwrap()
            .insert_cage(row_add(2, 0, 3))
            .unwrap()
            .insert_cage(row_add(2, 1, 3))
            .unwrap();
        let true_count = p.solutions();
        assert!(true_count >= 2);
        assert_eq!(p.solutions_at_most(1), 1);
        assert_eq!(p.solutions_at_most(2), 2);
        assert_eq!(p.solutions_at_most(100), true_count);
    }

    #[test]
    fn solutions_at_most_zero_for_overconstrained() {
        let p = Puzzle::new(2)
            .unwrap()
            .insert_cage(given(0, 0, 1))
            .unwrap()
            .insert_cage(given(0, 1, 1))
            .unwrap();
        assert_eq!(p.solutions_at_most(0), 0);
        assert_eq!(p.solutions_at_most(1), 0);
        assert_eq!(p.solutions_at_most(100), 0);
    }

    #[test]
    fn solve_2x2_partial_coverage_same_solutions_as_full_coverage() {
        // Only row 0 is caged; row 1 is unconstrained by any cage.
        // All-different alone determines the rest: same two solutions as the fully caged case.
        let puzzle = Puzzle::new(2)
            .unwrap()
            .insert_cage(row_add(2, 0, 3))
            .unwrap();
        assert_eq!(
            solve(puzzle),
            vec![vec![vec![2, 1], vec![1, 2]], vec![vec![1, 2], vec![2, 1]],]
        );
    }

    fn small_puzzle() -> Puzzle {
        Puzzle::new(2).unwrap().insert_cage(given(0, 0, 1)).unwrap()
    }

    #[test]
    fn grid_returns_underlying_grid() {
        let p = small_puzzle();
        assert_eq!(p.grid().n(), 2);
    }

    #[test]
    fn candidates_matches_grid_get() {
        let p = small_puzzle();
        let cell = Cell::new(0, 0);
        assert_eq!(p.candidates(cell).unwrap(), p.grid().get(&cell).unwrap());
    }

    #[test]
    fn candidates_out_of_bounds_is_err() {
        let p = small_puzzle();
        assert!(p.candidates(Cell::new(5, 5)).is_err());
    }

    #[test]
    fn cells_visits_every_cell_in_row_major_order() {
        let p = small_puzzle();
        let got: Vec<Cell> = p.cells().collect();
        assert_eq!(
            got,
            vec![
                Cell::new(0, 0),
                Cell::new(0, 1),
                Cell::new(1, 0),
                Cell::new(1, 1),
            ]
        );
    }

    #[test]
    fn singleton_cells_yields_only_singleton_cells() {
        // A fresh 1×1 puzzle has its lone cell as a singleton {1}.
        let p = Puzzle::new(1).unwrap();
        let got: Vec<(Cell, N)> = p.singleton_cells().collect();
        assert_eq!(got, vec![(Cell::new(0, 0), 1)]);
    }

    #[test]
    fn singleton_cells_empty_when_no_singletons() {
        // A fresh 2×2 puzzle has every cell containing {1, 2} — no singletons.
        let p = Puzzle::new(2).unwrap();
        assert_eq!(p.singleton_cells().count(), 0);
    }

    #[test]
    fn empty_cells_empty_for_fresh_puzzle() {
        let p = Puzzle::new(2).unwrap();
        assert_eq!(p.empty_cells().count(), 0);
    }

    #[test]
    fn empty_cells_yields_emptied_cell() {
        let mut p = Puzzle::new(2).unwrap();
        p.grid = p.grid.set(&Cell::new(1, 1), Values::default());
        let got: Vec<Cell> = p.empty_cells().collect();
        assert_eq!(got, vec![Cell::new(1, 1)]);
    }

    #[test]
    fn cage_at_returns_covering_cage() {
        let p = small_puzzle();
        let cage = p.cage_at(Cell::new(0, 0)).unwrap();
        assert_eq!(cage.operation(), Operation::Given(1));
    }

    #[test]
    fn cage_at_returns_none_for_uncovered_cell() {
        let p = small_puzzle();
        assert!(p.cage_at(Cell::new(1, 1)).is_none());
    }

    #[test]
    fn cages_iterates_every_cage() {
        let p = Puzzle::new(2)
            .unwrap()
            .insert_cage(given(0, 0, 1))
            .unwrap()
            .insert_cage(given(1, 1, 2))
            .unwrap();
        assert_eq!(p.cages().count(), 2);
    }

    #[test]
    fn is_covered_true_for_covered_cell() {
        let p = small_puzzle();
        assert!(p.is_covered(Cell::new(0, 0)));
    }

    #[test]
    fn is_covered_false_for_uncovered_cell() {
        let p = small_puzzle();
        assert!(!p.is_covered(Cell::new(1, 1)));
    }

    fn add_cage(cells: &[(usize, usize)], n: u8, target: u8) -> Cage {
        let cells: Vec<Cell> = cells
            .iter()
            .map(|&(row, column)| Cell::new(row, column))
            .collect();
        Cage::new(n, Polyomino::new(&cells), Operation::Add(target))
    }

    #[test]
    fn is_valid_true_for_fresh_puzzle() {
        assert!(Puzzle::new(2).unwrap().is_valid());
    }

    #[test]
    fn is_valid_false_when_a_cell_is_empty() {
        let mut p = Puzzle::new(2).unwrap();
        p.grid = p.grid.set(&Cell::new(0, 0), Values::default());
        assert!(!p.is_valid());
    }

    #[test]
    fn propagate_round_leaves_fresh_grid_unchanged() {
        let p = Puzzle::new(2).unwrap();
        let before = p.grid.clone();
        let after = p.propagate_round();
        assert_eq!(after.grid, before);
    }

    #[test]
    fn propagate_round_propagates_given_cage_value() {
        let p = Puzzle::new(2).unwrap().insert_cage(given(0, 0, 1)).unwrap();
        let after = p.propagate_round();
        assert_eq!(after.grid.get(&Cell::new(0, 0)).unwrap(), Values::new([1]));
    }

    #[test]
    fn propagate_round_yields_empty_for_overconstrained() {
        // A 2-cell Add cage with target 5 has no valid tuples in 1..=2,
        // so the cage filter pins both cells to the empty set.
        let p = Puzzle::new(2)
            .unwrap()
            .insert_cage(add_cage(&[(0, 0), (0, 1)], 2, 5))
            .unwrap();
        let after = p.propagate_round();
        assert!(after.grid.is_invalid());
        assert_eq!(after.grid.get(&Cell::new(0, 0)).unwrap(), Values::default());
    }

    #[test]
    fn propagate_fully_reaches_singletons_on_unique_solution() {
        // Given(0,0,1) on 2×2 forces every cell to a singleton via all-different.
        let p = Puzzle::new(2).unwrap().insert_cage(given(0, 0, 1)).unwrap();
        let fixed = p.propagate_fully();
        assert!(fixed.is_valid());
        for cell in fixed.cells() {
            assert!(fixed.grid.get(&cell).unwrap().is_singleton());
        }
    }

    #[test]
    fn propagate_fully_yields_invalid_for_overconstrained() {
        let p = Puzzle::new(2)
            .unwrap()
            .insert_cage(add_cage(&[(0, 0), (0, 1)], 2, 5))
            .unwrap();
        let fixed = p.propagate_fully();
        assert!(!fixed.is_valid());
    }

    #[test]
    fn propagate_fully_idempotent_at_fixed_point() {
        let p = Puzzle::new(2).unwrap().insert_cage(given(0, 0, 1)).unwrap();
        let once = p.propagate_fully();
        let twice = once.clone().propagate_fully();
        assert_eq!(once.grid, twice.grid);
    }

    #[test]
    fn narrow_with_identity_delta_is_no_op() {
        let p = Puzzle::new(2).unwrap().insert_cage(given(0, 0, 1)).unwrap();
        let baseline = p.clone().propagate_fully();
        let narrowed = p.narrow(&Delta::identity(2).unwrap());
        assert_eq!(narrowed.grid, baseline.grid);
    }

    #[test]
    fn narrow_with_singleton_delta_pins_cell() {
        // Pinning (0,0) to {1} on a fresh 2×2 grid forces (0,1)={2}, (1,0)={2}, (1,1)={1}.
        let p = Puzzle::new(2).unwrap();
        let delta = Delta::identity(2)
            .unwrap()
            .set(Cell::new(0, 0), Values::new([1]));
        let narrowed = p.narrow(&delta);
        assert!(narrowed.is_valid());
        assert_eq!(
            narrowed.grid.get(&Cell::new(0, 0)).unwrap(),
            Values::new([1])
        );
        assert_eq!(
            narrowed.grid.get(&Cell::new(0, 1)).unwrap(),
            Values::new([2])
        );
        assert_eq!(
            narrowed.grid.get(&Cell::new(1, 0)).unwrap(),
            Values::new([2])
        );
        assert_eq!(
            narrowed.grid.get(&Cell::new(1, 1)).unwrap(),
            Values::new([1])
        );
    }

    #[test]
    fn narrow_emptying_a_cell_yields_invalid() {
        let p = Puzzle::new(2).unwrap();
        let delta = Delta::identity(2)
            .unwrap()
            .set(Cell::new(0, 0), Values::default());
        let narrowed = p.narrow(&delta);
        assert!(!narrowed.is_valid());
    }

    #[test]
    fn widen_with_identity_delta_is_no_op() {
        let p = Puzzle::new(2).unwrap().insert_cage(given(0, 0, 1)).unwrap();
        let baseline = p.clone().propagate_fully();
        let widened = p.widen(&Delta::identity(2).unwrap());
        assert_eq!(widened.grid, baseline.grid);
    }

    #[test]
    fn widen_re_narrows_to_constraint_implications() {
        // Start from the fully-propagated puzzle (every cell singleton). Widen with the
        // identity delta unions full {1,2} into every cell, then propagation re-narrows
        // to the same fixed point.
        let p = Puzzle::new(2).unwrap().insert_cage(given(0, 0, 1)).unwrap();
        let baseline = p.propagate_fully();
        let widened = baseline.widen(&Delta::identity(2).unwrap());
        assert!(widened.is_valid());
        for cell in widened.cells() {
            assert!(widened.grid.get(&cell).unwrap().is_singleton());
        }
    }

    #[test]
    #[should_panic(expected = "delta size")]
    fn narrow_panics_on_size_mismatch() {
        let p = Puzzle::new(2).unwrap();
        let _ = p.narrow(&Delta::identity(3).unwrap());
    }

    #[test]
    #[should_panic(expected = "delta size")]
    fn widen_panics_on_size_mismatch() {
        let p = Puzzle::new(2).unwrap();
        let _ = p.widen(&Delta::identity(3).unwrap());
    }
}
