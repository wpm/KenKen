use std::collections::BTreeSet;

use crate::{
    Cage, Cell, Cover, Error,
    Error::CageConflict,
    Fill, Grid, State,
    constraints::{Constraint, all_different::AllDifferent},
};

#[derive(Debug, Clone)]
pub struct Puzzle {
    grid: Grid,
    all_different: Vec<AllDifferent>,
    cages: BTreeSet<Cage>,
}

impl Puzzle {
    /// Creates an `n`×`n` puzzle with no cages and all-different constraints
    /// on every row and column.
    ///
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`.
    pub fn new(n: usize) -> Result<Self, Error> {
        Ok(Self {
            grid: Grid::new(n)?,
            all_different: vec![AllDifferent::row, AllDifferent::column]
                .into_iter()
                .flat_map(|f| (0..n).map(move |i| f(n, i)))
                .collect::<Result<_, _>>()?,
            cages: BTreeSet::default(),
        })
    }

    // TODO Puzzle.set() should propagate.
    fn set(&self, grid: Grid) -> Self {
        Self {
            grid,
            all_different: self.all_different.clone(),
            cages: self.cages.clone(),
        }
    }

    /// Returns a new [`Puzzle`] with `cage` added. The puzzle's grid will reflect the constraints
    /// imposed by the new cage.
    ///
    /// This is idempotent. Adding a cage identical to one already present returns the puzzle
    /// unchanged.
    ///
    /// # Errors
    /// Returns [`CageConflict`] if `cage` overlaps any cage already in
    /// the puzzle not identical to `cage`.
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

    /// Returns a new puzzle with `cage` removed.
    ///
    /// This is idempotent. Attempting to remove a `cage` that is not present returns the puzzle
    /// unchanged.
    #[must_use]
    pub fn remove(&self, cage: &Cage) -> Self {
        let mut cages = self.cages.clone();
        if !cages.remove(cage) {
            return self.clone();
        }
        Self {
            grid: self.grid.clone(),
            all_different: self.all_different.clone(),
            cages,
        }
    }

    #[cfg(test)]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Applies every constraint once, folding them left over the current grid.
    ///
    /// Order is arbitrary: all constraints are monotone filters (they only
    /// remove candidates, never add them), so application order does not affect
    /// the fixed-point result.
    fn apply_constraints(&self) -> Result<Grid, Error> {
        self.all_different
            .iter()
            .map(|c| c as &dyn Constraint)
            .chain(self.cages.iter().map(|c| c as &dyn Constraint))
            .try_fold(self.grid.clone(), |grid, c| c.apply_to(&grid))
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
            let grid = puzzle.apply_constraints()?;
            if grid.is_invalid() {
                return Ok(None);
            }
            if grid == puzzle.grid {
                break;
            }
            puzzle = puzzle.set(grid);
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
                fill.iter()
                    .map(move |v| self.set(grid.clone().set(&cell, Fill::new([v]))))
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
        Cage, Cell, Cover, Error, Fill, Operation, Polyomino, State,
        constraints::test_utils::{cells, singleton},
    };

    fn puzzle_4() -> Puzzle {
        Puzzle::new(4).unwrap()
    }

    fn singleton_cage() -> Cage {
        Cage::new(4, singleton(), Operation::Given(3))
    }

    // --- Puzzle::new ---

    #[test]
    fn new_invalid_size_returns_err() {
        assert!(Puzzle::new(0).is_err());
        assert!(Puzzle::new(10).is_err());
    }

    #[test]
    fn new_valid_size_succeeds() {
        assert!(Puzzle::new(1).is_ok());
        assert!(Puzzle::new(9).is_ok());
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
        let p2 = p.remove(&cage);
        // Can insert the same cage again after removal
        assert!(p2.insert(cage).is_ok());
    }

    #[test]
    fn remove_absent_cage_is_noop() {
        let p = puzzle_4();
        let p2 = p.remove(&singleton_cage());
        assert!(p2.insert(singleton_cage()).is_ok());
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
        let result = Puzzle::new(2)
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
