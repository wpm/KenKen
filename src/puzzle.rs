#![allow(dead_code)]
use crate::constraints::{AllDifferent, Cage, Cages, PuzzleConstraints};
use crate::grid::Grid;
use crate::shape::Polyomino;
use crate::solver::{Solver, State};
use crate::types::{Error, Index, Values};
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
    pub(crate) fn insert_cage(mut self, cage: Cage) -> Result<Self, Error> {
        let constraints = Arc::make_mut(&mut self.constraints);
        constraints.cage = constraints.cage.clone().insert(cage)?;
        Ok(self)
    }

    /// Returns a new puzzle with the cage removed.
    ///
    /// Idempotent: if no such cage exists, returns unchanged.
    pub(crate) fn remove_cage(mut self, polyomino: &Polyomino) -> Self {
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
        let mut iter = Solver::new(self.clone());
        match (iter.next(), iter.next()) {
            (None, _) => Uniqueness::None,
            (Some(_), None) => Uniqueness::Unique,
            (Some(_), Some(_)) => Uniqueness::Multiple,
        }
    }

    /// Counts every solution by exhaustive search.
    ///
    /// Use [`Puzzle::uniqueness`] when only the bucket (none / one / many) is needed.
    #[must_use]
    pub fn solutions(&self) -> usize {
        Solver::new(self.clone()).count()
    }
}

impl State for Puzzle {
    fn propagate(self) -> Option<Self> {
        let filter = self.constraints.apply(&self.grid).ok()?;
        let grid = filter.apply(&self.grid).ok()?;
        if grid.is_invalid() {
            return None;
        }
        Some(Self {
            grid,
            constraints: self.constraints,
        })
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
    use crate::constraints::{Constraint, Operation};
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
}
