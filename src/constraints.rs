#![allow(dead_code)]

use crate::Error::InvalidCell;
use crate::arithmetic::cage_tuples;
use crate::shape::Cells;
use crate::{Cell, Error, Grid, Index, M, N, Polyomino, Values};
use itertools::Itertools;
use std::collections::HashMap;
use std::ops::BitAnd;

/// A `Constraint` is a function that selects candidate values to remove from a `Grid`.
pub trait Constraint {
    /// # Errors
    /// Returns `Err` if a cell referenced by this constraint is not present in the grid.
    fn apply(&self, grid: &Grid) -> Result<ValueFilter, Error>;
}

/// A constraint that ensures that each cell in a row or column has a different value.
#[must_use]
#[derive(Debug, Clone)]
pub struct AllDifferent {
    values: Vec<Cell>,
}

impl AllDifferent {
    /// Creates a row constraint for `row` in an `n`×`n` grid.
    pub fn row(n: Index, row: Index) -> Self {
        Self {
            values: (0..n).map(|column| Cell::new(row, column)).collect(),
        }
    }

    /// Creates a column constraint for `column` in an `n`×`n` grid.
    pub fn column(n: Index, column: Index) -> Self {
        Self {
            values: (0..n).map(|row| Cell::new(row, column)).collect(),
        }
    }
}

impl Constraint for AllDifferent {
    #[allow(clippy::todo)]
    fn apply(&self, grid: &Grid) -> Result<ValueFilter, Error> {
        let _values = self.values.iter().map(|cell| grid.get(cell));
        todo!("Run Régin's algorithm and write the result into the map.")
    }
}

/// A polyomino constraint defined by a set of cells and the set of valid value assignments.
#[must_use]
#[derive(Debug, Clone)]
pub struct Cage {
    polyomino: Polyomino,
    operation: Operation,
    tuples: Vec<Vec<N>>,
}

impl Cage {
    /// Creates a polyomino over the given cells, computing valid tuples from the operation.
    pub fn new(n: N, polyomino: Polyomino, operation: Operation) -> Self {
        let tuples = cage_tuples(n, polyomino.len(), &operation);
        Self {
            polyomino,
            operation,
            tuples,
        }
    }

    #[must_use]
    pub const fn polyomino(&self) -> &Polyomino {
        &self.polyomino
    }

    #[must_use]
    pub fn cells(&self) -> Vec<Cell> {
        self.polyomino.cells()
    }

    fn grid_values(&self, grid: &Grid) -> Result<Vec<Values>, Error> {
        self.cells().iter().map(|cell| grid.get(cell)).collect()
    }
}

impl Constraint for Cage {
    /// Constrains each cell to the union of values it takes across all valid tuple assignments.
    fn apply(&self, grid: &Grid) -> Result<ValueFilter, Error> {
        let cells = self.cells();
        let grid_values = self.grid_values(grid)?;
        let valid_tuples = filter_tuples(&grid_values, &self.tuples);
        let valid_tuple_values = transpose(&valid_tuples);
        Ok(ValueFilter(
            cells.into_iter().zip(valid_tuple_values).collect(),
        ))
    }
}

/// Filters tuples to those consistent with the current grid values.
/// A tuple is consistent if each of its values appears in the corresponding cell's candidate set.
fn filter_tuples(cage_values: &[Values], tuples: &[Vec<N>]) -> Vec<Vec<N>> {
    tuples
        .iter()
        .filter(|tuple| {
            cage_values
                .iter()
                .zip(tuple.iter())
                .all(|(v, t)| v.iter().contains(t))
        })
        .cloned()
        .collect()
}

/// Transposes the tuple matrix: collects the union of values at each position across all tuples.
/// Position `i` in the result is the set of all values that appear at index `i` in any tuple.
fn transpose(tuples: &[Vec<N>]) -> Vec<Values> {
    assert!(!tuples.is_empty(), "Cannot transpose empty tuple matrix");
    let cage_size = tuples[0].len();
    tuples
        .iter()
        .fold(vec![Values::default(); cage_size], |mut cols, tuple| {
            for (col, val) in cols.iter_mut().zip(tuple.iter()) {
                *col = *col | Values::new([*val]);
            }
            cols
        })
}

/// An arithmetic operation that defines a polyomino.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Operation {
    Add(N),
    Subtract(N),
    Multiply(M),
    Divide(N),
    Given(N),
}

/// A partial map from cells to candidate values representing a constraint's effect on a grid.
/// Cells absent from the filter are unconstrained.
pub struct ValueFilter(pub HashMap<Cell, Values>);

impl ValueFilter {
    /// Returns the candidate values for `cell`, or `Err` if the cell is not in this filter.
    fn get(&self, cell: Cell) -> Result<Values, Error> {
        self.0.get(&cell).copied().ok_or(InvalidCell(cell))
    }

    fn apply(&self, grid: &Grid) -> Result<Grid, Error> {
        let mut grid = grid.clone();
        for (cell, u) in &self.0 {
            let v = grid.get(cell)?;
            grid = grid.set(cell, *u & v);
        }
        Ok(grid)
    }
}

impl BitAnd for ValueFilter {
    type Output = Self;

    /// Intersects two filters: keeps only cells present in both, with values narrowed to their
    /// intersection. Cells whose intersection is empty are dropped.
    fn bitand(self, rhs: Self) -> Self::Output {
        let map = self
            .0
            .into_iter()
            .filter_map(|(cell, lv)| {
                let rv = rhs.0.get(&cell)?;
                let intersection = lv & *rv;
                if intersection == Values::default() {
                    None
                } else {
                    Some((cell, intersection))
                }
            })
            .collect();
        Self(map)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::Index;

    fn cell(row: Index, column: Index) -> Cell {
        Cell::new(row, column)
    }

    #[test]
    fn line_row_has_correct_cells() {
        let row = AllDifferent::row(4, 2);
        for c in 0..4 {
            assert!(row.values.contains(&cell(2, c)));
        }
    }

    #[test]
    fn line_column_has_correct_cells() {
        let column = AllDifferent::column(4, 1);
        for r in 0..4 {
            assert!(column.values.contains(&cell(r, 1)));
        }
    }

    #[test]
    fn transpose_tuples_unions_columns() {
        let tuples = vec![vec![1, 2], vec![2, 3]];
        let result = transpose(&tuples);
        assert_eq!(result[0], Values::new([1, 2]));
        assert_eq!(result[1], Values::new([2, 3]));
    }

    #[test]
    fn transpose_single_tuple() {
        let result = transpose(&[vec![1, 2, 3]]);
        assert_eq!(result[0], Values::new([1]));
        assert_eq!(result[1], Values::new([2]));
        assert_eq!(result[2], Values::new([3]));
    }
    fn filter(pairs: &[(Cell, Values)]) -> ValueFilter {
        ValueFilter(pairs.iter().copied().collect())
    }

    #[test]
    fn value_filter_get_present_cell() {
        let f = filter(&[(cell(0, 0), Values::new([1, 2]))]);
        assert_eq!(f.get(cell(0, 0)).unwrap(), Values::new([1, 2]));
    }

    #[test]
    fn value_filter_get_missing_cell_is_err() {
        let f = filter(&[]);
        assert!(f.get(cell(0, 0)).is_err());
    }

    #[test]
    fn value_filter_mul_intersects_values() {
        let a = filter(&[(cell(0, 0), Values::new([1, 2, 3]))]);
        let b = filter(&[(cell(0, 0), Values::new([2, 3, 4]))]);
        let result = a & b;
        assert_eq!(result.0[&cell(0, 0)], Values::new([2, 3]));
    }

    #[test]
    fn value_filter_mul_drops_cells_not_in_both() {
        let a = filter(&[
            (cell(0, 0), Values::new([1, 2])),
            (cell(0, 1), Values::new([1])),
        ]);
        let b = filter(&[(cell(0, 0), Values::new([1, 2]))]);
        let result = a & b;
        assert!(result.0.contains_key(&cell(0, 0)));
        assert!(!result.0.contains_key(&cell(0, 1)));
    }

    #[test]
    fn value_filter_mul_drops_empty_intersection() {
        let a = filter(&[(cell(0, 0), Values::new([1, 2]))]);
        let b = filter(&[(cell(0, 0), Values::new([3, 4]))]);
        let result = a & b;
        assert!(!result.0.contains_key(&cell(0, 0)));
    }
}
