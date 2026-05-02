#![allow(dead_code)]

use crate::Error::InvalidCell;
use crate::arithmetic::cage_tuples;
use crate::regin::regin;
use crate::{Cell, Error, Grid, Index, M, N, Polyomino, Values};
use itertools::Itertools;
use std::collections::HashMap;
use std::ops::Mul;

/// A `Constraint` creates a [`ValueFilter`] that narrows candidate values in a [`Grid`].
pub trait Constraint {
    fn cells(&self) -> &[Cell];

    /// # Errors
    /// Returns `Error` if any cell in this constraint is outside the grid.
    fn grid_value_filter(&self, grid: &Grid) -> Result<ValueFilter, Error>;

    /// # Errors
    /// Returns `Error` if any cell in this constraint is outside the grid.
    fn grid_cell_values(&self, grid: &Grid) -> Result<Vec<Values>, Error> {
        self.cells().iter().map(|cell| grid.get(cell)).collect()
    }
}

/// A constraint that ensures each cell in a row or column has a different value.
#[must_use]
#[derive(Debug, Clone)]
pub struct AllDifferent(Vec<Cell>);

impl AllDifferent {
    /// Creates a row constraint for `row` in an `n`×`n` grid.
    pub fn row(n: Index, row: Index) -> Self {
        Self((0..n).map(|column| Cell::new(row, column)).collect())
    }

    /// Creates a column constraint for `column` in an `n`×`n` grid.
    pub fn column(n: Index, column: Index) -> Self {
        Self((0..n).map(|row| Cell::new(row, column)).collect())
    }
}

impl Constraint for AllDifferent {
    fn cells(&self) -> &[Cell] {
        &self.0
    }

    fn grid_value_filter(&self, grid: &Grid) -> Result<ValueFilter, Error> {
        let grid_values = self.grid_cell_values(grid)?;
        let all_different_values = regin(&grid_values);
        let cells = self.cells();
        Ok(ValueFilter(
            cells.iter().copied().zip(all_different_values).collect(),
        ))
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

    pub const fn polyomino(&self) -> &Polyomino {
        &self.polyomino
    }
}

impl Constraint for Cage {
    fn cells(&self) -> &[Cell] {
        self.polyomino.as_slice()
    }

    /// Constrains each cell to the union of values it takes across all valid tuple assignments.
    fn grid_value_filter(&self, grid: &Grid) -> Result<ValueFilter, Error> {
        let grid_values = self.grid_cell_values(grid)?;
        let valid_tuples = filter_tuples(&grid_values, &self.tuples);
        let valid_tuple_values = transpose(self.cells().len(), &valid_tuples);
        let cells = self.cells();
        Ok(ValueFilter(
            cells.iter().copied().zip(valid_tuple_values).collect(),
        ))
    }
}

/// Filters tuples to those consistent with the current grid values.
///
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

/// Transposes the tuple matrix: position `i` in the result is the union of values at index `i`
/// across all tuples.
fn transpose(cage_size: usize, tuples: &[Vec<N>]) -> Vec<Values> {
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

/// A partial map from cells to candidate values representing a constraint's effect on a grid;
/// cells absent from the filter are unconstrained.
#[derive(Debug, Clone, Default)]
pub struct ValueFilter(pub HashMap<Cell, Values>);

impl ValueFilter {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Returns the candidate values for `cell`, or `Error` if the cell is not in this filter.
    fn get(&self, cell: Cell) -> Result<Values, Error> {
        self.0.get(&cell).copied().ok_or(InvalidCell(cell))
    }

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

/// The set of constraints for a single KenKen puzzle.
///
/// Stored behind an [`Arc`] so that cloning a [`Puzzle`] during search shares this allocation
/// instead of duplicating it.
#[derive(Debug, Clone)]
pub struct PuzzleConstraints {
    pub row: Vec<AllDifferent>,
    pub column: Vec<AllDifferent>,
    pub cage: Cages,
}

impl PuzzleConstraints {
    /// # Errors
    /// Returns `Error` if any cell in any constraint is outside the grid.
    pub fn apply(&self, grid: &Grid) -> Result<ValueFilter, Error> {
        let grid = grid.clone();
        let a = self.cage_filter(&grid)?;
        let b = self.all_different_filter(&grid)?;
        Ok(a * b)
    }
    fn cage_filter(&self, grid: &Grid) -> Result<ValueFilter, Error> {
        let mut filter = ValueFilter::default();
        for cage in self.cage.values() {
            filter = filter * cage.grid_value_filter(grid)?;
        }
        Ok(filter)
    }
    fn all_different_filter(&self, grid: &Grid) -> Result<ValueFilter, Error> {
        let mut filter = ValueFilter::default();
        for row in &self.row {
            filter = filter * row.grid_value_filter(grid)?;
        }
        for column in &self.column {
            filter = filter * column.grid_value_filter(grid)?;
        }
        Ok(filter)
    }
}

/// The set of [`Cage`] constraints for a puzzle, keyed by polyomino.
///
/// A cell may belong to at most one cage. Conflict detection on insert is O(n) in the number
/// of cages, which is acceptable because cages are only added at construction time.
#[must_use]
#[derive(Debug, Clone, Default)]
pub struct Cages(HashMap<Polyomino, Cage>);

impl Cages {
    /// Returns a new cage set with the cage inserted.
    ///
    /// Idempotent: if the exact same cage (by polyomino) is already present, returns unchanged.
    /// # Errors
    /// Returns `Error` if any cell in the cage is already claimed by a *different* cage.
    pub fn insert(mut self, cage: Cage) -> Result<Self, Error> {
        if self.0.contains_key(cage.polyomino()) {
            return Ok(self);
        }
        let new_cells: std::collections::HashSet<Cell> = cage.cells().iter().copied().collect();
        let existing = self
            .0
            .values()
            .find(|c| c.cells().iter().any(|cell| new_cells.contains(cell)));
        if let Some(existing) = existing {
            return Err(Error::CageConflict(
                Box::new(cage),
                Box::new(existing.clone()),
            ));
        }
        self.0.insert(cage.polyomino().clone(), cage);
        Ok(self)
    }

    /// Returns a new cage set with the cage removed.
    ///
    /// Idempotent: if no such cage exists, returns unchanged.
    pub fn remove(mut self, polyomino: &Polyomino) -> Self {
        self.0.remove(polyomino);
        self
    }

    #[must_use]
    pub fn get(&self, polyomino: &Polyomino) -> Option<&Cage> {
        self.0.get(polyomino)
    }

    #[must_use]
    pub fn contains(&self, polyomino: &Polyomino) -> bool {
        self.0.contains_key(polyomino)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn values(&self) -> impl Iterator<Item = &Cage> {
        self.0.values()
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
            assert!(row.cells().contains(&cell(2, c)));
        }
    }

    #[test]
    fn line_column_has_correct_cells() {
        let column = AllDifferent::column(4, 1);
        for r in 0..4 {
            assert!(column.cells().contains(&cell(r, 1)));
        }
    }

    #[test]
    fn transpose_tuples_unions_columns() {
        let tuples = vec![vec![1, 2], vec![2, 3]];
        let result = transpose(2, &tuples);
        assert_eq!(result[0], Values::new([1, 2]));
        assert_eq!(result[1], Values::new([2, 3]));
    }

    #[test]
    fn transpose_single_tuple() {
        let result = transpose(3, &[vec![1, 2, 3]]);
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
    fn value_filter_mul_intersects_values_for_shared_cell() {
        let a = filter(&[(cell(0, 0), Values::new([1, 2, 3]))]);
        let b = filter(&[(cell(0, 0), Values::new([2, 3, 4]))]);
        let result = a * b;
        assert_eq!(result.0[&cell(0, 0)], Values::new([2, 3]));
    }

    #[test]
    fn value_filter_mul_keeps_cells_from_both_sides() {
        let a = filter(&[
            (cell(0, 0), Values::new([1, 2])),
            (cell(0, 1), Values::new([1])),
        ]);
        let b = filter(&[(cell(0, 0), Values::new([1, 2]))]);
        let result = a * b;
        assert!(result.0.contains_key(&cell(0, 0)));
        assert!(result.0.contains_key(&cell(0, 1)));
    }

    #[test]
    fn value_filter_mul_keeps_empty_intersection_as_contradiction() {
        let a = filter(&[(cell(0, 0), Values::new([1, 2]))]);
        let b = filter(&[(cell(0, 0), Values::new([3, 4]))]);
        let result = a * b;
        assert!(result.0.contains_key(&cell(0, 0)));
        assert_eq!(result.0[&cell(0, 0)], Values::default());
    }

    #[test]
    fn value_filter_mul_disjoint_cells_keeps_all() {
        let a = filter(&[(cell(0, 0), Values::new([1, 2]))]);
        let b = filter(&[(cell(1, 1), Values::new([3, 4]))]);
        let result = a * b;
        assert!(result.0.contains_key(&cell(0, 0)));
        assert!(result.0.contains_key(&cell(1, 1)));
    }

    #[test]
    fn value_filter_mul_overlap_and_disjoint() {
        let a = filter(&[
            (cell(0, 0), Values::new([1, 2, 3])),
            (cell(0, 1), Values::new([1, 2])),
        ]);
        let b = filter(&[
            (cell(0, 0), Values::new([2, 3, 4])),
            (cell(1, 0), Values::new([1])),
        ]);
        let result = a * b;
        assert_eq!(result.0[&cell(0, 0)], Values::new([2, 3]));
        assert_eq!(result.0[&cell(0, 1)], Values::new([1, 2]));
        assert_eq!(result.0[&cell(1, 0)], Values::new([1]));
    }
}
