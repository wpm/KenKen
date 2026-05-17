use std::collections::HashMap;

use crate::{
    Error::InvalidCell,
    constraints::{Cover, all_different::AllDifferent},
    types::{Cell, Error, Fill},
};

/// A square n×n arrangement of cells, each of which has a [`Fill`] of candidate
/// values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    n: usize,
    // Stored as a single flat allocation of `n×n` [`Fill`]s in row-major order, so cloning is a
    // single `memcpy`.
    fills: Box<[Fill]>,
}

impl Grid {
    /// Creates an n×n grid where every cell is filled with `1..=n`.
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`.
    pub fn new(n: usize) -> Result<Self, Error> {
        if !(1..=9).contains(&n) {
            return Err(Error::InvalidGridSize(n));
        }
        Ok(Self {
            n,
            fills: vec![Fill::full(n); n * n].into_boxed_slice(),
        })
    }

    /// The number of rows or columns in the grid.
    pub const fn n(&self) -> usize {
        self.n
    }

    /// Returns all-different constraints for every row and column of this grid.
    ///
    /// # Errors
    /// Returns [`Error`] if the grid size is invalid (structurally unreachable for
    /// any `Grid` constructed via [`Grid::new`]).
    pub fn all_different_constraints(&self) -> Result<Vec<AllDifferent>, Error> {
        [
            AllDifferent::row as fn(usize, usize) -> _,
            AllDifferent::column,
        ]
        .into_iter()
        .flat_map(|f| (0..self.n).map(move |i| f(self.n, i)))
        .collect()
    }

    /// Returns the candidate values for `cell`.
    /// # Errors
    /// Returns [`InvalidCell`] if `cell` is outside the grid bounds.
    pub fn get(&self, cell: &Cell) -> Result<Fill, Error> {
        self.fill_index(cell)
            .map(|i| self.fills[i])
            .ok_or(InvalidCell(*cell))
    }

    /// Changes the fill of `cell` in the grid to `fill` and returns the grid.
    pub fn set(mut self, cell: &Cell, fill: Fill) -> Self {
        if let Some(i) = self.fill_index(cell) {
            self.fills[i] = fill;
        }
        self
    }

    /// Returns a new grid with each cell's candidates intersected with the
    /// corresponding [`Fill`] in `fill_constraints`. Cells not present in the
    /// map are left unchanged.
    ///
    /// # Errors
    /// Returns [`InvalidCell`] if any key in `fill_constraints` is
    /// outside this grid's bounds.
    pub fn apply(&self, fill_constraints: HashMap<Cell, Fill>) -> Result<Self, Error> {
        let original = self.clone();
        fill_constraints
            .into_iter()
            .try_fold(self.clone(), |grid, (cell, fill)| {
                Ok(grid.set(&cell, original.get(&cell)? & fill))
            })
    }

    /// Has every cell fill been narrowed to a single value?
    pub fn is_solved(&self) -> bool {
        self.fills.iter().all(|values| values.is_singleton())
    }

    /// Is any cell fill empty?
    pub fn is_invalid(&self) -> bool {
        self.fills.iter().any(|values| values.is_empty())
    }

    const fn fill_index(&self, cell: &Cell) -> Option<usize> {
        if cell.row < self.n && cell.column < self.n {
            Some(cell.row * self.n + cell.column)
        } else {
            None
        }
    }
}

impl Cover for Grid {
    fn cells(&self) -> impl Iterator<Item = Cell> {
        (0..self.n).flat_map(|row| (0..self.n).map(move |column| Cell::new(row, column)))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn all_different_constraints_count_is_2n() {
        let g = Grid::new(4).unwrap();
        assert_eq!(g.all_different_constraints().unwrap().len(), 8);
    }

    #[test]
    fn all_different_constraints_covers_every_cell_exactly_twice() {
        // Each cell belongs to exactly one row constraint and one column constraint.
        let g = Grid::new(3).unwrap();
        let constraints = g.all_different_constraints().unwrap();
        for row in 0..3 {
            for col in 0..3 {
                let cell = Cell::new(row, col);
                let count = constraints
                    .iter()
                    .filter(|c| c.cells().any(|c| c == cell))
                    .count();
                assert_eq!(
                    count, 2,
                    "cell ({row},{col}) should appear in exactly 2 constraints"
                );
            }
        }
    }

    #[test]
    fn all_different_constraints_row_cells_share_row() {
        let g = Grid::new(3).unwrap();
        let constraints = g.all_different_constraints().unwrap();
        // First n constraints are rows: all cells in each have the same row index.
        for c in constraints.iter().take(3) {
            let rows: Vec<usize> = c.cells().map(|cell| cell.row).collect();
            assert!(rows.windows(2).all(|w| w[0] == w[1]));
        }
    }

    #[test]
    fn all_different_constraints_column_cells_share_column() {
        let g = Grid::new(3).unwrap();
        let constraints = g.all_different_constraints().unwrap();
        // Last n constraints are columns: all cells in each have the same column index.
        for c in constraints.iter().skip(3) {
            let cols: Vec<usize> = c.cells().map(|cell| cell.column).collect();
            assert!(cols.windows(2).all(|w| w[0] == w[1]));
        }
    }

    #[test]
    fn new_returns_err_for_zero() {
        assert!(Grid::new(0).is_err());
    }

    #[test]
    fn new_returns_err_for_ten() {
        assert!(Grid::new(10).is_err());
    }

    #[test]
    fn new_grid_has_correct_dimensions() {
        let g = Grid::new(4).unwrap();
        assert_eq!(g.n(), 4);
        assert_eq!(g.fills.len(), 16);
    }

    #[test]
    fn new_grid_cells_have_full_values() {
        let g = Grid::new(3).unwrap();
        assert!(g.fills.iter().all(|v| *v == Fill::full(3)));
    }

    #[test]
    fn get_valid_cell_returns_values() {
        let g = Grid::new(3).unwrap();
        assert_eq!(g.get(&Cell::new(0, 0)).unwrap(), Fill::full(3));
    }

    #[test]
    fn get_invalid_cell_returns_err() {
        let g = Grid::new(3).unwrap();
        assert!(g.get(&Cell::new(9, 9)).is_err());
    }

    #[test]
    fn set_updates_cell_value() {
        let g = Grid::new(3).unwrap();
        let cell = Cell::new(1, 2);
        let new_values = Fill::new([1, 2]);
        let g2 = g.set(&cell, new_values);
        assert_eq!(g2.get(&cell).unwrap(), new_values);
    }

    #[test]
    fn set_leaves_other_cells_unchanged() {
        let g = Grid::new(3).unwrap();
        let cell = Cell::new(0, 0);
        let other = Cell::new(1, 1);
        let g2 = g.set(&cell, Fill::new([1]));
        assert_eq!(g2.get(&other).unwrap(), Fill::full(3));
    }

    #[test]
    fn clone_is_independent() {
        let g = Grid::new(3).unwrap();
        let cell = Cell::new(0, 0);
        let g2 = g.clone().set(&cell, Fill::new([1]));
        assert_eq!(g.get(&cell).unwrap(), Fill::full(3));
        assert_eq!(g2.get(&cell).unwrap(), Fill::new([1]));
    }

    #[test]
    fn cells_are_contiguous() {
        let g = Grid::new(3).unwrap();
        assert_eq!(g.fills.len(), 9);
    }

    fn solved_grid(n: usize) -> Grid {
        let mut g = Grid::new(n).unwrap();
        for row in 0..n {
            for col in 0..n {
                g = g.set(
                    &Cell::new(row, col),
                    Fill::new([u8::try_from((row + col) % n + 1).unwrap()]),
                );
            }
        }
        g
    }

    #[test]
    fn is_solved_false_for_fresh_grid() {
        assert!(!Grid::new(3).unwrap().is_solved());
    }

    #[test]
    fn is_solved_true_when_all_cells_are_singletons() {
        assert!(solved_grid(3).is_solved());
    }

    #[test]
    fn is_solved_false_when_one_cell_has_multiple_values() {
        let g = solved_grid(3).set(&Cell::new(0, 0), Fill::new([1, 2]));
        assert!(!g.is_solved());
    }

    #[test]
    fn is_invalid_false_for_fresh_grid() {
        assert!(!Grid::new(3).unwrap().is_invalid());
    }

    #[test]
    fn is_invalid_false_for_solved_grid() {
        assert!(!solved_grid(3).is_invalid());
    }

    #[test]
    fn is_invalid_true_when_one_cell_is_empty() {
        let g = Grid::new(3).unwrap().set(&Cell::new(1, 1), Fill::default());
        assert!(g.is_invalid());
    }

    #[test]
    fn is_invalid_true_when_all_cells_are_empty() {
        let mut g = Grid::new(2).unwrap();
        for row in 0..2 {
            for col in 0..2 {
                g = g.set(&Cell::new(row, col), Fill::default());
            }
        }
        assert!(g.is_invalid());
    }

    #[test]
    fn set_out_of_bounds_is_noop() {
        let g = Grid::new(3).unwrap();
        let before = g.clone();
        let after = g.set(&Cell::new(9, 9), Fill::new([1]));
        assert_eq!(after, before);
    }

}
