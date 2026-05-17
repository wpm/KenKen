//! A [`Puzzle`] pairs a candidate [`Grid`] with a set of [`Cage`] constraints
//! and all-different constraints for every row and column.

use std::collections::BTreeSet;

use crate::{
    Cage, Cell, Cover, Error,
    Error::{CageConflict, CageNotInPuzzle},
    Fill, Grid, State,
    constraints::{Constraint, all_different::AllDifferent},
};

/// A KenKen puzzle: a [`Grid`] of candidate values together with cage and
/// all-different constraints.
///
/// ## Fixpoint invariant
///
/// A *fixpoint* is a state in which applying all constraints produces no
/// further change — every cell's candidate set is already as narrow as the
/// constraints require. Every `Puzzle` upholds this invariant: construction
/// and mutation methods ([`new`](Puzzle::new), [`new_empty`](Puzzle::new_empty),
/// [`insert`](Puzzle::insert), `set_grid`) propagate constraints to
/// fixpoint before returning. If propagation would empty any cell's candidate
/// set (a contradiction), the method returns `None` instead of a `Puzzle`,
/// so a `Puzzle` value always represents a consistent, fully propagated state.
#[derive(Debug, Clone)]
pub struct Puzzle {
    grid: Grid,
    all_different: Vec<AllDifferent>,
    cages: BTreeSet<Cage>,
}

impl Puzzle {
    /// Creates an `n`×`n` puzzle with no cages.
    ///
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`.
    pub fn new_empty(n: usize) -> Result<Self, Error> {
        let grid = Grid::new(n)?;
        Ok(Self {
            grid: grid.clone(),
            all_different: grid.all_different_constraints()?,
            cages: BTreeSet::default(),
        })
    }

    /// Creates a puzzle from an existing grid and a set of cages, then propagates
    /// all constraints. Returns `None` if propagation finds a contradiction.
    ///
    /// # Errors
    /// Returns [`CageNotInPuzzle`] if any cage contains a cell outside the grid.
    /// Returns [`Error`] if propagation encounters a cell outside the grid bounds.
    pub fn new(grid: &Grid, cages: &[Cage]) -> Result<Option<Self>, Error> {
        let n = grid.n();
        for cage in cages {
            if cage.cells().any(|c| c.row >= n || c.column >= n) {
                return Err(CageNotInPuzzle(cage.clone()));
            }
        }
        Self {
            grid: grid.clone(),
            all_different: grid.all_different_constraints()?,
            cages: cages.iter().cloned().collect(),
        }
        .propagate()
    }

    /// Returns a new puzzle with `grid` substituted in place of the current grid,
    /// then propagates all constraints. Returns `None` if propagation finds a
    /// contradiction. Cages and all-different constraints are carried over unchanged.
    fn set_grid(&self, grid: Grid) -> Result<Option<Self>, Error> {
        Self {
            grid,
            all_different: self.all_different.clone(),
            cages: self.cages.clone(),
        }
        .propagate()
    }

    /// Returns a new [`Puzzle`] with `cage` added. The puzzle's grid will reflect the constraints
    /// imposed by the new cage. If this results in an invalid [`Grid`] state, this function
    /// will return `none`.
    ///
    /// This is idempotent. Adding a cage identical to one already present returns the puzzle
    /// unchanged.
    ///
    /// # Errors
    /// Returns [`CageConflict`] if `cage` overlaps any cage already in
    /// the puzzle not identical to `cage`.
    #[allow(unused_results)]
    pub fn insert(&self, cage: Cage) -> Result<Option<Self>, Error> {
        if self.cages.contains(&cage) {
            return Ok(Some(self.clone()));
        }
        if self
            .cages
            .iter()
            .any(|puzzle_cage| puzzle_cage.polyomino().intersects(cage.polyomino()))
        {
            return Err(CageConflict(cage));
        }
        let mut cages = self.cages.clone();
        cages.insert(cage);
        Self {
            grid: self.grid.clone(),
            all_different: self.all_different.clone(),
            cages,
        }
        .propagate()
    }

    /// Returns a new puzzle with `cage` removed and constraints re-propagated.
    ///
    /// The entire grid is reset to `Fill::full(n)` before re-propagating with the remaining
    /// cages, so all constraints determine their new domains from scratch.
    ///
    /// This is idempotent. Attempting to remove a `cage` that is not present returns
    /// the puzzle unchanged.
    ///
    /// # Errors
    /// Returns [`Error`] if propagation encounters a cell outside the grid bounds.
    pub fn remove(&self, cage: &Cage) -> Result<Self, Error> {
        let mut cages = self.cages.clone();
        if !cages.remove(cage) {
            return Ok(self.clone());
        }
        let n = self.grid.n();
        Ok(Self {
            grid: Grid::new(n)?,
            all_different: self.all_different.clone(),
            cages,
        }
        .propagate()?
        // Safe: Grid::new(n) succeeds because n came from a valid Puzzle (n in 1..=9),
        // and filling every cell to full before propagating can only widen domains, never empty
        // them.
        .unwrap_or_else(|| unreachable!("widening fills cannot produce a contradiction")))
    }

    /// Returns the puzzle's cages in ascending [`Cage`] order — by polyomino
    /// cells (row-major), then operation, then tuples.
    pub fn cages(&self) -> impl Iterator<Item = &Cage> {
        self.cages.iter()
    }

    #[cfg(test)]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Applies every constraint once, folding them left over the current grid.
    /// Returns `None` if any cell's fill becomes empty after applying constraints.
    ///
    /// Order is arbitrary: all constraints are monotone filters (they only
    /// remove candidates, never add them), so application order does not affect
    /// the fixed-point result.
    fn apply_constraints(&self) -> Result<Option<Grid>, Error> {
        self.all_different
            .iter()
            .map(|c| c as &dyn Constraint)
            .chain(self.cages.iter().map(|c| c as &dyn Constraint))
            .try_fold(self.grid.clone(), |grid, c| c.apply_to(&grid))
            .map(|grid| (!grid.is_invalid()).then_some(grid))
    }
}

impl State for Puzzle {
    /// Applies all constraints repeatedly until the grid stabilizes or a
    /// contradiction is found.
    ///
    /// Returns `None` if any cell's fill becomes empty (indicating that no solution
    /// exists). Returns `Some(puzzle)` otherwise.
    ///
    /// # Errors
    /// Returns [`Error`] if a constraint references a cell outside the grid.
    fn propagate(&self) -> Result<Option<Self>, Error> {
        let mut puzzle = self.clone();
        loop {
            let Some(grid) = puzzle.apply_constraints()? else {
                return Ok(None);
            };
            if grid == puzzle.grid {
                break;
            }
            match puzzle.set_grid(grid)? {
                Some(p) => puzzle = p,
                None => return Ok(None),
            }
        }
        Ok(Some(puzzle))
    }

    /// Returns one child puzzle per candidate value of the most-constrained
    /// cell — the cell with the fewest remaining candidates, breaking ties by
    /// cell order. Each child has that cell pinned to a single value.
    ///
    /// Returns an empty iterator if the grid has no cells, signaling a solution.
    fn branch(&self) -> impl Iterator<Item = Self> {
        self.grid
            .most_constrained()
            .into_iter()
            .flat_map(move |(cell, fill)| {
                let grid = self.grid.clone();
                fill.iter().filter_map(move |v| {
                    self.set_grid(grid.clone().set(&cell, Fill::new([v])))
                        .ok()
                        .flatten()
                })
            })
    }
}

impl Cover for Puzzle {
    fn cells(&self) -> impl Iterator<Item = Cell> {
        self.grid.cells()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::Puzzle;
    use crate::{
        Cage, Cell, Cover, Error, Fill, Grid, Operation, Polyomino, State,
        constraints::test_utils::{cells, singleton},
    };

    fn puzzle_4() -> Puzzle {
        Puzzle::new_empty(4).unwrap()
    }

    fn singleton_cage() -> Cage {
        Cage::new(4, singleton(), Operation::Given(3))
    }

    // --- Puzzle::new ---

    #[test]
    fn new_cage_outside_grid_returns_err() {
        // A 2-cell cage whose second cell lies outside the 1×1 grid.
        let grid = Grid::new(1).unwrap();
        let out_of_bounds = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(0, 0), (0, 1)])).unwrap(),
            Operation::Add(3),
        );
        assert!(matches!(
            Puzzle::new(&grid, &[out_of_bounds]),
            Err(Error::CageNotInPuzzle(_))
        ));
    }

    #[test]
    fn new_empty_invalid_size_returns_err() {
        assert!(Puzzle::new_empty(0).is_err());
        assert!(Puzzle::new_empty(10).is_err());
    }

    #[test]
    fn new_empty_valid_size_succeeds() {
        assert!(Puzzle::new_empty(1).is_ok());
        assert!(Puzzle::new_empty(9).is_ok());
    }

    #[test]
    fn new_empty_cells_have_full_candidates() {
        let puzzle = Puzzle::new_empty(3).unwrap();
        assert!(
            puzzle
                .cells()
                .all(|c| puzzle.grid().get(&c).unwrap() == Fill::full(3))
        );
    }

    #[test]
    fn new_with_no_cages_returns_some() {
        let grid = Grid::new(4).unwrap();
        assert!(Puzzle::new(&grid, &[]).unwrap().is_some());
    }

    #[test]
    fn new_with_valid_cage_returns_some_and_propagates() {
        let grid = Grid::new(4).unwrap();
        let cage = Cage::new(4, singleton(), Operation::Given(2));
        let puzzle = Puzzle::new(&grid, &[cage]).unwrap().unwrap();
        assert_eq!(puzzle.grid().get(&Cell::new(0, 0)).unwrap(), Fill::new([2]));
    }

    #[test]
    fn new_with_contradicting_cages_returns_none() {
        // Two Given(1) cages in the same row of a 2×2 grid: AllDifferent makes it unsolvable.
        let grid = Grid::new(2).unwrap();
        let c1 = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(0, 0)])).unwrap(),
            Operation::Given(1),
        );
        let c2 = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(0, 1)])).unwrap(),
            Operation::Given(1),
        );
        assert!(Puzzle::new(&grid, &[c1, c2]).unwrap().is_none());
    }

    // --- Puzzle::insert ---

    #[test]
    fn insert_non_overlapping_cage_succeeds() {
        let p = puzzle_4().insert(singleton_cage());
        assert!(p.is_ok());
    }

    #[test]
    fn insert_overlapping_cage_returns_err() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let overlap = Cage::new(4, singleton(), Operation::Given(1));
        assert!(matches!(p.insert(overlap), Err(Error::CageConflict(_))));
    }

    #[test]
    fn insert_is_non_destructive() {
        let base = puzzle_4();
        let _ = base.insert(singleton_cage()).unwrap();
        // base is unchanged — inserting into it again still succeeds
        assert!(base.insert(singleton_cage()).is_ok());
    }

    #[test]
    fn insert_duplicate_cage_is_idempotent() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let p2 = p.insert(singleton_cage()).unwrap().unwrap();
        assert_eq!(p.grid(), p2.grid());
    }

    // --- Puzzle::remove ---

    #[test]
    fn remove_present_cage_returns_puzzle_without_it() {
        let cage = singleton_cage();
        let p = puzzle_4().insert(cage.clone()).unwrap().unwrap();
        let p2 = p.remove(&cage).unwrap();
        // Can insert the same cage again after removal
        assert!(p2.insert(cage).is_ok());
    }

    #[test]
    fn remove_absent_cage_is_noop() {
        let p = puzzle_4();
        let p2 = p.remove(&singleton_cage()).unwrap();
        assert!(p2.insert(singleton_cage()).is_ok());
    }

    #[test]
    fn remove_resets_cells_to_full() {
        // Insert Given(3) at (0,0), which pins that cell to {3}.
        // After removal, (0,0) should be widened back to the full candidate set.
        let cage = singleton_cage(); // Given(3) at (0,0)
        let p = puzzle_4().insert(cage.clone()).unwrap().unwrap();
        assert_eq!(p.grid().get(&Cell::new(0, 0)).unwrap(), Fill::new([3]));
        let p2 = p.remove(&cage).unwrap();
        assert_eq!(p2.grid().get(&Cell::new(0, 0)).unwrap(), Fill::full(4));
    }

    // --- Puzzle::cages ---

    #[test]
    fn cages_fresh_puzzle_is_empty() {
        assert_eq!(puzzle_4().cages().count(), 0);
    }

    #[test]
    fn cages_yields_in_storage_order() {
        let a = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(0, 0)])).unwrap(),
            Operation::Given(3),
        );
        let b = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(1, 1)])).unwrap(),
            Operation::Given(2),
        );
        // Insert b first so storage order (BTreeSet by Cage::Ord) differs from insertion order.
        let puzzle = puzzle_4()
            .insert(b.clone())
            .unwrap()
            .unwrap()
            .insert(a.clone())
            .unwrap()
            .unwrap();
        itertools::assert_equal(puzzle.cages(), &[a, b]);
    }

    // --- Puzzle::branch ---

    #[test]
    fn branch_fresh_4x4_yields_four_children() {
        // Fresh 4×4: every cell has fill {1,2,3,4}, so the most-constrained cell has 4 candidates.
        assert_eq!(puzzle_4().branch().count(), 4);
    }

    #[test]
    fn branch_children_each_propagate_without_error() {
        for child in puzzle_4().branch() {
            assert!(child.propagate().is_ok());
        }
    }

    #[test]
    fn branch_after_given_cage_yields_one_child() {
        // Given(3) pins (0,0) to {3}; it is the most-constrained cell (size 1).
        let puzzle = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        assert_eq!(puzzle.branch().count(), 1);
    }

    #[test]
    fn branch_children_pin_most_constrained_cell_to_distinct_singleton_values() {
        let puzzle = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let mut values: Vec<Fill> = puzzle
            .branch()
            .map(|child| {
                let (_, fill) = child.grid().most_constrained().unwrap();
                fill
            })
            .collect();
        // Each branch pins to exactly one value.
        assert!(values.iter().all(|f| f.len() == 1));
        // All pinned values are distinct.
        values.sort_by_key(|f| f.iter().next().unwrap());
        values.dedup();
        assert_eq!(values.len(), puzzle.branch().count());
    }

    // --- Puzzle::set_grid ---

    #[test]
    fn set_grid_replaces_grid_and_propagates() {
        // Pin (0,0) to {2} in a fresh grid and set it into a puzzle that has a
        // Given(2) cage at (0,0). Propagation should confirm the cell stays {2}.
        let cage = Cage::new(4, singleton(), Operation::Given(2));
        let puzzle = puzzle_4().insert(cage).unwrap().unwrap();
        let new_grid = puzzle.grid().clone().set(&Cell::new(0, 0), Fill::new([2]));
        let result = puzzle.set_grid(new_grid).unwrap().unwrap();
        assert_eq!(result.grid().get(&Cell::new(0, 0)).unwrap(), Fill::new([2]));
    }

    #[test]
    fn set_grid_preserves_cages() {
        // After set_grid, a cage that was present before is still enforced.
        let cage = singleton_cage(); // Given(3) at (0,0)
        let puzzle = puzzle_4().insert(cage).unwrap().unwrap();
        let same_grid = puzzle.grid().clone();
        let result = puzzle.set_grid(same_grid).unwrap().unwrap();
        assert_eq!(result.grid().get(&Cell::new(0, 0)).unwrap(), Fill::new([3]));
    }

    #[test]
    fn set_grid_returns_none_on_contradiction() {
        // Force an empty fill on a cell — propagation should detect it as invalid.
        let puzzle = puzzle_4();
        let contradicting_grid = puzzle.grid().clone().set(&Cell::new(0, 0), Fill::default());
        assert!(puzzle.set_grid(contradicting_grid).unwrap().is_none());
    }

    // --- Puzzle::propagate ---

    #[test]
    fn propagate_fresh_puzzle_returns_some() {
        assert!(puzzle_4().propagate().unwrap().is_some());
    }

    #[test]
    fn propagate_with_given_cage_narrows_cell() {
        let cage = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(0, 0)])).unwrap(),
            Operation::Given(2),
        );
        let puzzle = puzzle_4().insert(cage).unwrap().unwrap();
        let result = puzzle.propagate().unwrap().unwrap();
        assert_eq!(result.grid().get(&Cell::new(0, 0)).unwrap(), Fill::new([2]));
    }

    #[test]
    fn propagate_contradiction_returns_none() {
        // Two Given(1) cages in the same row forces two cells to {1},
        // which AllDifferent will eliminate to empty — a contradiction.
        // propagate() is called inside insert(), so insert() itself returns None.
        let c1 = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(0, 0)])).unwrap(),
            Operation::Given(1),
        );
        let c2 = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(0, 1)])).unwrap(),
            Operation::Given(1),
        );
        let result = Puzzle::new_empty(2)
            .unwrap()
            .insert(c1)
            .unwrap()
            .unwrap()
            .insert(c2)
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn cover_cells_count_equals_n_squared() {
        assert_eq!(puzzle_4().cells().count(), 16);
    }

    #[test]
    fn propagate_is_idempotent() {
        let cage = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(0, 0)])).unwrap(),
            Operation::Given(2),
        );
        let puzzle = puzzle_4().insert(cage).unwrap().unwrap();
        let once = puzzle.propagate().unwrap().unwrap();
        let twice = once.propagate().unwrap().unwrap();
        assert_eq!(once.grid(), twice.grid());
    }
}
