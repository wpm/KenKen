//! `Delta` is an overlay of candidate-value sets used to transform a [`crate::Puzzle`]'s
//! grid via [`crate::Puzzle::narrow`] (per-cell intersection) or
//! [`crate::Puzzle::widen`] (per-cell union).

#![allow(dead_code)]

use crate::geometry::grid::Grid;
use crate::types::{Cell, Error, Index, Values};

/// An overlay of candidate-value sets, shaped like a [`Grid`] but distinguished from it
/// by the type system: a `Delta` is a transform, not a state.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delta(Grid);

impl Delta {
    /// Identity delta for an n×n grid: every cell is `Values::full(n)`.
    ///
    /// # Errors
    /// Returns `Error` if `n` is not in `1..=9`.
    pub fn identity(n: Index) -> Result<Self, Error> {
        Grid::new(n).map(Self)
    }

    /// The side length of the underlying grid.
    #[must_use]
    pub const fn n(&self) -> Index {
        self.0.n()
    }

    /// Returns a new `Delta` with `cell` set to `values`.
    pub fn set(self, cell: Cell, values: Values) -> Self {
        Self(self.0.set(&cell, values))
    }

    /// Returns the values at `cell`.
    ///
    /// # Errors
    /// Returns `Error` if `cell` is outside the grid.
    pub fn get(&self, cell: Cell) -> Result<Values, Error> {
        self.0.get(&cell)
    }

    pub(crate) const fn grid(&self) -> &Grid {
        &self.0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn identity_returns_err_for_invalid_size() {
        assert!(Delta::identity(0).is_err());
        assert!(Delta::identity(10).is_err());
    }

    #[test]
    fn identity_every_cell_is_full() {
        let d = Delta::identity(3).unwrap();
        for row in 0..3 {
            for column in 0..3 {
                assert_eq!(d.get(Cell::new(row, column)).unwrap(), Values::full(3));
            }
        }
    }

    #[test]
    fn n_matches_constructor_argument() {
        assert_eq!(Delta::identity(4).unwrap().n(), 4);
    }

    #[test]
    fn set_overrides_target_cell() {
        let d = Delta::identity(3)
            .unwrap()
            .set(Cell::new(1, 2), Values::new([1]));
        assert_eq!(d.get(Cell::new(1, 2)).unwrap(), Values::new([1]));
    }

    #[test]
    fn set_leaves_other_cells_unchanged() {
        let d = Delta::identity(3)
            .unwrap()
            .set(Cell::new(1, 2), Values::new([1]));
        assert_eq!(d.get(Cell::new(0, 0)).unwrap(), Values::full(3));
    }

    #[test]
    fn get_returns_err_for_out_of_bounds_cell() {
        let d = Delta::identity(3).unwrap();
        assert!(d.get(Cell::new(9, 9)).is_err());
    }

    #[test]
    fn get_returns_full_for_unset_cell_on_identity() {
        let d = Delta::identity(2).unwrap();
        assert_eq!(d.get(Cell::new(0, 1)).unwrap(), Values::full(2));
    }
}
