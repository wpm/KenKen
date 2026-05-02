#![allow(dead_code)]

use crate::Error::InvalidCell;
use crate::types::{Cell, Error, Values};

/// A KenKen grid mapping each cell to its set of candidate values.
///
/// Stored as a single flat allocation of `n×n` [`Values`] in row-major order,
/// so cloning is one `memcpy`.
#[must_use]
#[derive(Debug, Clone)]
pub struct Grid {
    n: usize,
    cells: Box<[Values]>,
}

impl Grid {
    /// Creates an n×n grid where every cell contains the values `1..=n`.
    /// # Errors
    /// Returns `Error` if `n` is not in `1..=9`.
    pub fn new(n: usize) -> Result<Self, Error> {
        if !(1..=9).contains(&n) {
            return Err(Error::InvalidGridSize(n));
        }
        Ok(Self {
            n,
            cells: vec![Values::full(n); n * n].into_boxed_slice(),
        })
    }

    /// The number of rows and columns in the grid.
    #[must_use]
    pub const fn n(&self) -> usize {
        self.n
    }

    /// Returns the candidate values for `cell`.
    /// # Errors
    /// Returns `Error` if `cell` is outside the grid bounds.
    pub fn get(&self, cell: &Cell) -> Result<Values, Error> {
        self.index(cell)
            .map(|i| self.cells[i])
            .ok_or(InvalidCell(*cell))
    }

    /// Returns a new grid with `cell` set to `values`.
    pub(crate) fn set(mut self, cell: &Cell, values: Values) -> Self {
        if let Some(i) = self.index(cell) {
            self.cells[i] = values;
        }
        self
    }

    #[must_use]
    pub fn is_solved(&self) -> bool {
        self.cells.iter().all(|values| values.is_singleton())
    }

    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.cells.iter().any(|values| values.is_empty())
    }

    /// Iterates over all cells in row-major order.
    pub fn iter(&self) -> impl Iterator<Item = Cell> {
        let n = self.n;
        (0..n).flat_map(move |row| (0..n).map(move |column| Cell::new(row, column)))
    }

    const fn index(&self, cell: &Cell) -> Option<usize> {
        if cell.row < self.n && cell.column < self.n {
            Some(cell.row * self.n + cell.column)
        } else {
            None
        }
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
        assert_eq!(g.cells.len(), 16);
    }

    #[test]
    fn new_grid_cells_have_full_values() {
        let g = Grid::new(3).unwrap();
        assert!(g.cells.iter().all(|v| *v == Values::full(3)));
    }

    #[test]
    fn get_valid_cell_returns_values() {
        let g = Grid::new(3).unwrap();
        assert_eq!(g.get(&Cell::new(0, 0)).unwrap(), Values::full(3));
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
        let new_values = Values::new([1, 2]);
        let g2 = g.set(&cell, new_values);
        assert_eq!(g2.get(&cell).unwrap(), new_values);
    }

    #[test]
    fn set_leaves_other_cells_unchanged() {
        let g = Grid::new(3).unwrap();
        let cell = Cell::new(0, 0);
        let other = Cell::new(1, 1);
        let g2 = g.set(&cell, Values::new([1]));
        assert_eq!(g2.get(&other).unwrap(), Values::full(3));
    }

    #[test]
    fn clone_is_independent() {
        let g = Grid::new(3).unwrap();
        let cell = Cell::new(0, 0);
        let g2 = g.clone().set(&cell, Values::new([1]));
        assert_eq!(g.get(&cell).unwrap(), Values::full(3));
        assert_eq!(g2.get(&cell).unwrap(), Values::new([1]));
    }

    #[test]
    fn cells_are_contiguous() {
        let g = Grid::new(3).unwrap();
        assert_eq!(g.cells.len(), 9);
    }

    fn solved_grid(n: usize) -> Grid {
        let mut g = Grid::new(n).unwrap();
        for row in 0..n {
            for col in 0..n {
                g = g.set(
                    &Cell::new(row, col),
                    Values::new([u8::try_from((row + col) % n + 1).unwrap()]),
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
        let g = solved_grid(3).set(&Cell::new(0, 0), Values::new([1, 2]));
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
        let g = Grid::new(3)
            .unwrap()
            .set(&Cell::new(1, 1), Values::default());
        assert!(g.is_invalid());
    }

    #[test]
    fn is_invalid_true_when_all_cells_are_empty() {
        let mut g = Grid::new(2).unwrap();
        for row in 0..2 {
            for col in 0..2 {
                g = g.set(&Cell::new(row, col), Values::default());
            }
        }
        assert!(g.is_invalid());
    }
}
