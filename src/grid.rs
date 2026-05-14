#![allow(clippy::must_use_candidate)]

use crate::{
    Error::InvalidCell,
    constraints::{all_different::AllDifferent, cover::Cover},
    types::{Cell, Error, Fill},
};

/// A square n×n arrangement of cells, each of which has a [`Fill`] of candidate
/// values.
#[must_use]
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

    /// The number of rows and columns in the grid.
    #[must_use]
    pub const fn n(&self) -> usize {
        self.n
    }

    /// Returns `true` if every cell has been narrowed to a single value.
    #[must_use]
    pub fn is_solved(&self) -> bool {
        self.fills.iter().all(|values| values.is_singleton())
    }

    /// Returns `true` if any cell has no remaining candidate values.
    pub fn is_invalid(&self) -> bool {
        self.fills.iter().any(|values| values.is_empty())
    }

    /// Creates an [`AllDifferent`] for the `index`th row of the grid.
    /// # Errors
    /// Returns [`Error::IndexOutOfRange`] if `index` is not less than `n`.
    pub fn row(&self, index: usize) -> Result<AllDifferent, Error> {
        AllDifferent::row(self.n, index)
    }

    /// Creates an [`AllDifferent`] for the `index`th column of the grid.
    /// # Errors
    /// Returns [`Error::IndexOutOfRange`] if `index` is not less than `n`.
    pub fn column(&self, index: usize) -> Result<AllDifferent, Error> {
        AllDifferent::column(self.n, index)
    }

    /// Returns the candidate values for `cell`.
    /// # Errors
    /// Returns [`Error::InvalidCell`] if `cell` is outside the grid bounds.
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

    /// Returns an iterator over every `(cell, fill)` pair in row-major order.
    pub fn entries(&self) -> impl Iterator<Item = (Cell, Fill)> + '_ {
        self.cells()
            .into_iter()
            .map(|cell| (cell, self.fills[cell.row * self.n + cell.column]))
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
    fn cells(&self) -> Vec<Cell> {
        (0..self.n)
            .flat_map(|row| (0..self.n).map(move |column| Cell::new(row, column)))
            .collect()
    }

    fn len(&self) -> usize {
        self.cells().len()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

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

    #[test]
    fn entries_count_equals_n_squared() {
        let g = Grid::new(3).unwrap();
        assert_eq!(g.entries().count(), 9);
    }

    #[test]
    fn entries_are_in_row_major_order() {
        let g = Grid::new(2).unwrap();
        let cells: Vec<Cell> = g.entries().map(|(c, _)| c).collect();
        assert_eq!(
            cells,
            vec![
                Cell::new(0, 0),
                Cell::new(0, 1),
                Cell::new(1, 0),
                Cell::new(1, 1),
            ]
        );
    }

    #[test]
    fn entries_reflect_set_values() {
        let cell = Cell::new(1, 2);
        let g = Grid::new(3).unwrap().set(&cell, Fill::new([2]));
        let fill = g.entries().find(|(c, _)| c == &cell).map(|(_, f)| f);
        assert_eq!(fill, Some(Fill::new([2])));
    }

    #[test]
    fn entries_covers_all_cells() {
        let n = 4;
        let g = Grid::new(n).unwrap();
        let mut seen: std::collections::HashSet<Cell> = std::collections::HashSet::new();
        for (cell, _) in g.entries() {
            assert!(seen.insert(cell), "duplicate cell {cell:?}");
        }
        assert_eq!(seen.len(), n * n);
    }

    #[test]
    fn row_returns_all_different_for_row() {
        use crate::constraints::cover::Cover;
        let g = Grid::new(3).unwrap();
        let r = g.row(1).unwrap();
        assert_eq!(
            r.cells(),
            vec![Cell::new(1, 0), Cell::new(1, 1), Cell::new(1, 2)]
        );
    }

    #[test]
    fn row_out_of_bounds_returns_err() {
        let g = Grid::new(3).unwrap();
        assert!(g.row(3).is_err());
    }

    #[test]
    fn column_returns_all_different_for_column() {
        use crate::constraints::cover::Cover;
        let g = Grid::new(3).unwrap();
        let c = g.column(2).unwrap();
        assert_eq!(
            c.cells(),
            vec![Cell::new(0, 2), Cell::new(1, 2), Cell::new(2, 2)]
        );
    }

    #[test]
    fn column_out_of_bounds_returns_err() {
        let g = Grid::new(3).unwrap();
        assert!(g.column(3).is_err());
    }

    #[test]
    fn len_equals_n_squared() {
        use crate::constraints::cover::Cover;
        let g = Grid::new(4).unwrap();
        assert_eq!(g.len(), 16);
    }
}
