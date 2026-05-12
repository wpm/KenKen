#![allow(dead_code)]

use crate::geometry::grid::Grid;
use crate::geometry::shape::Cells;
use crate::types::Error::InvalidCell;
use crate::types::{Cell, Error, Values};
use std::collections::HashMap;
use std::ops::Mul;

/// A `Constraint` creates a [`ValueFilter`] that removes candidate values from a [`Grid`].
pub trait Constraint: Cells {
    /// Returns a filter expressing which values this constraint permits at each of its cells,
    /// given the current candidate sets in `grid`.
    ///
    /// The filter maps each cell to the subset of its current candidates that is consistent
    /// with at least one valid assignment for this constraint. Applying the filter narrows
    /// the grid; if any cell's set becomes empty, the grid is contradicted.
    ///
    /// # Errors
    /// Returns `Error` if any cell in this constraint is outside the grid.
    fn value_filter(&self, grid: &Grid) -> Result<ValueFilter, Error>;
}

/// A map from cells to candidate values representing a constraint's effect on a grid;
/// cells absent from the filter are unconstrained.
#[derive(Debug, Clone, Default)]
pub struct ValueFilter(pub HashMap<Cell, Values>);

impl ValueFilter {
    /// The allowed values for `cell`, or `Error` if the cell is not in this filter.
    fn get(&self, cell: Cell) -> Result<Values, Error> {
        self.0.get(&cell).copied().ok_or(InvalidCell(cell))
    }

    /// Returns a new grid with each cell's candidates intersected with this filter's values.
    ///
    /// Cells absent from the filter are left unchanged. A cell whose intersection becomes empty
    /// is kept as-is (empty set); callers can detect this via [`Grid::is_invalid`].
    ///
    /// # Errors
    /// Returns `Error` if any cell in this filter is outside the grid.
    pub fn apply(&self, grid: &Grid) -> Result<Grid, Error> {
        let mut grid = grid.clone();
        for (cell, u) in &self.0 {
            let v = grid.get(cell)?;
            grid = grid.set(cell, *u & v);
        }
        Ok(grid)
    }
}

impl FromIterator<(Cell, Values)> for ValueFilter {
    fn from_iter<T: IntoIterator<Item = (Cell, Values)>>(iter: T) -> Self {
        let mut map = HashMap::new();
        for (cell, values) in iter {
            map.insert(cell, values);
        }
        Self(map)
    }
}

impl Mul for ValueFilter {
    type Output = Self;

    /// Merges two filters: union of cell sets, intersecting values where both have an opinion.
    /// Cells present in only one filter pass through unchanged.
    /// Empty intersections are kept so that `Grid::is_invalid` can detect contradictions.
    fn mul(self, rhs: Self) -> Self::Output {
        let mut map = self.0;
        for (cell, rv) in rhs.0 {
            map.entry(cell)
                .and_modify(|lv| *lv = *lv & rv)
                .or_insert(rv);
        }
        Self(map)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::constraints::constraint::ValueFilter;
    use crate::{Cell, Index, Values};

    fn filter(pairs: &[(Cell, Values)]) -> ValueFilter {
        ValueFilter(pairs.iter().copied().collect())
    }

    #[test]
    fn value_filter_get_present_cell() {
        let f = filter(&[(Cell::new(0, 0), Values::new([1, 2]))]);
        assert_eq!(f.get(Cell::new(0, 0)).unwrap(), Values::new([1, 2]));
    }

    #[test]
    fn value_filter_get_missing_cell_is_err() {
        let f = filter(&[]);
        assert!(f.get(Cell::new(0, 0)).is_err());
    }

    #[test]
    fn value_filter_mul_intersects_values_for_shared_cell() {
        let a = filter(&[(Cell::new(0, 0), Values::new([1, 2, 3]))]);
        let b = filter(&[(Cell::new(0, 0), Values::new([2, 3, 4]))]);
        let result = a * b;
        assert_eq!(result.0[&Cell::new(0, 0)], Values::new([2, 3]));
    }

    #[test]
    fn value_filter_mul_keeps_cells_from_both_sides() {
        let a = filter(&[
            (Cell::new(0, 0), Values::new([1, 2])),
            (Cell::new(0, 1), Values::new([1])),
        ]);
        let b = filter(&[(Cell::new(0, 0), Values::new([1, 2]))]);
        let result = a * b;
        assert!(result.0.contains_key(&Cell::new(0, 0)));
        assert!(result.0.contains_key(&Cell::new(0, 1)));
    }

    #[test]
    fn value_filter_mul_keeps_empty_intersection_as_contradiction() {
        let a = filter(&[(Cell::new(0, 0), Values::new([1, 2]))]);
        let b = filter(&[(Cell::new(0, 0), Values::new([3, 4]))]);
        let result = a * b;
        assert!(result.0.contains_key(&Cell::new(0, 0)));
        assert_eq!(result.0[&Cell::new(0, 0)], Values::default());
    }

    #[test]
    fn value_filter_mul_disjoint_cells_keeps_all() {
        let a = filter(&[(Cell::new(0, 0), Values::new([1, 2]))]);
        let b = filter(&[(Cell::new(1, 1), Values::new([3, 4]))]);
        let result = a * b;
        assert!(result.0.contains_key(&Cell::new(0, 0)));
        assert!(result.0.contains_key(&Cell::new(1, 1)));
    }

    fn cells_of(positions: &[(Index, Index)]) -> Vec<Cell> {
        positions.iter().map(|&(r, c)| Cell::new(r, c)).collect()
    }
    #[test]
    fn value_filter_mul_overlap_and_disjoint() {
        let a = filter(&[
            (Cell::new(0, 0), Values::new([1, 2, 3])),
            (Cell::new(0, 1), Values::new([1, 2])),
        ]);
        let b = filter(&[
            (Cell::new(0, 0), Values::new([2, 3, 4])),
            (Cell::new(1, 0), Values::new([1])),
        ]);
        let result = a * b;
        assert_eq!(result.0[&Cell::new(0, 0)], Values::new([2, 3]));
        assert_eq!(result.0[&Cell::new(0, 1)], Values::new([1, 2]));
        assert_eq!(result.0[&Cell::new(1, 0)], Values::new([1]));
    }
}
