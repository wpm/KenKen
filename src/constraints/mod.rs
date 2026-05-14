use std::{collections::HashMap, ops::Mul};

#[allow(dead_code)]
use crate::grid::Grid;
use crate::{Cell, Error, Fill};

pub mod all_different;
pub mod cage;
pub mod cover;
pub mod regin;
pub mod tiling;

/// A constraint that can be applied to a region of cells.
pub trait RegionConstraint {
    fn constraint(&self, grid: &Grid) -> Constraint;
}

/// A constraint on the [`Fill`]s for a region of [`Cell`]s. This consists of a
/// set of cells and allowed fills for them.
#[derive(Default)]
pub struct Constraint(HashMap<Cell, Fill>);

impl Constraint {
    /// When applied to a [`Grid`], the constraint restricts the [`Fill`]s for
    /// the specified cells.
    ///
    /// # Errors
    /// Returns `Error` if any cell in the constraint is outside the grid
    /// bounds.
    pub fn apply(&self, grid: Grid) -> Result<Grid, Error> {
        let g = grid.clone();
        self.0.iter().try_fold(grid, |grid, (cell, values)| {
            Ok(grid.set(cell, *values & g.get(cell)?))
        })
    }
}

impl Mul for Constraint {
    type Output = Self;

    /// Merges two constraints, creating a union of the cell sets and
    /// intersecting values. Cells present in only one filter pass through
    /// unchanged. Empty intersections are kept so that `Grid::is_invalid`
    /// can detect contradictions.
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

impl FromIterator<(Cell, Fill)> for Constraint {
    fn from_iter<T: IntoIterator<Item = (Cell, Fill)>>(iter: T) -> Self {
        let mut map = HashMap::new();
        for (cell, values) in iter {
            map.insert(cell, values);
        }
        Self(map)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::{Cell, Error::InvalidCell, Fill, constraints::Constraint, grid::Grid};

    fn constraint(pairs: &[(Cell, Fill)]) -> Constraint {
        Constraint(pairs.iter().copied().collect())
    }

    #[test]
    fn value_filter_mul_intersects_values_for_shared_cell() {
        let a = constraint(&[(Cell::new(0, 0), Fill::new([1, 2, 3]))]);
        let b = constraint(&[(Cell::new(0, 0), Fill::new([2, 3, 4]))]);
        let result = a * b;
        assert_eq!(result.0[&Cell::new(0, 0)], Fill::new([2, 3]));
    }

    #[test]
    fn value_filter_mul_keeps_cells_from_both_sides() {
        let a = constraint(&[
            (Cell::new(0, 0), Fill::new([1, 2])),
            (Cell::new(0, 1), Fill::new([1])),
        ]);
        let b = constraint(&[(Cell::new(0, 0), Fill::new([1, 2]))]);
        let result = a * b;
        assert!(result.0.contains_key(&Cell::new(0, 0)));
        assert!(result.0.contains_key(&Cell::new(0, 1)));
    }

    #[test]
    fn value_filter_mul_keeps_empty_intersection_as_contradiction() {
        let a = constraint(&[(Cell::new(0, 0), Fill::new([1, 2]))]);
        let b = constraint(&[(Cell::new(0, 0), Fill::new([3, 4]))]);
        let result = a * b;
        assert!(result.0.contains_key(&Cell::new(0, 0)));
        assert_eq!(result.0[&Cell::new(0, 0)], Fill::default());
    }

    #[test]
    fn cells_outside_grid_return_error() {
        let f = constraint(&[(Cell::new(9, 9), Fill::new([1]))]);
        let grid = Grid::new(3).unwrap();
        let after = f.apply(grid);
        assert!(matches!(after, Err(InvalidCell(_))));
    }

    #[test]
    fn value_filter_mul_disjoint_cells_keeps_all() {
        let a = constraint(&[(Cell::new(0, 0), Fill::new([1, 2]))]);
        let b = constraint(&[(Cell::new(1, 1), Fill::new([3, 4]))]);
        let result = a * b;
        assert!(result.0.contains_key(&Cell::new(0, 0)));
        assert!(result.0.contains_key(&Cell::new(1, 1)));
    }

    #[test]
    fn value_filter_mul_overlap_and_disjoint() {
        let a = constraint(&[
            (Cell::new(0, 0), Fill::new([1, 2, 3])),
            (Cell::new(0, 1), Fill::new([1, 2])),
        ]);
        let b = constraint(&[
            (Cell::new(0, 0), Fill::new([2, 3, 4])),
            (Cell::new(1, 0), Fill::new([1])),
        ]);
        let result = a * b;
        assert_eq!(result.0[&Cell::new(0, 0)], Fill::new([2, 3]));
        assert_eq!(result.0[&Cell::new(0, 1)], Fill::new([1, 2]));
        assert_eq!(result.0[&Cell::new(1, 0)], Fill::new([1]));
    }
}
