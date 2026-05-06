#![allow(dead_code)]

use crate::arithmetic::cage_tuples;
use crate::grid::Grid;
use crate::regin::regin;
use crate::shape::Polyomino;
use crate::tiling::Tiling;
use crate::types::Error::InvalidCell;
use crate::types::{Cell, Error, Index, M, N, Values};
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
    /// Tuples that violate all-different on any row or column group of cage cells are dropped.
    pub fn new(n: N, polyomino: Polyomino, operation: Operation) -> Self {
        let tuples = cage_tuples(n, polyomino.len(), operation);
        let groups = row_and_column_groups(polyomino.as_slice());
        let tuples = tuples
            .into_iter()
            .filter(|t| groups.iter().all(|g| values_all_different(t, g)))
            .collect();
        Self {
            polyomino,
            operation,
            tuples,
        }
    }

    pub const fn polyomino(&self) -> &Polyomino {
        &self.polyomino
    }

    /// Returns a copy of this cage's [`Operation`].
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }

    /// Returns a slice over the cage's precomputed valid value tuples.
    #[must_use]
    pub fn tuples(&self) -> &[Vec<N>] {
        &self.tuples
    }

    /// Returns the cells covered by this cage.
    ///
    /// Inherent counterpart of the internal `Constraint::cells` so callers can read a
    /// cage's cells without importing the (crate-private) `Constraint` trait. The body
    /// delegates to that trait method so the two paths share a single source of truth.
    pub fn cells(&self) -> &[Cell] {
        <Self as Constraint>::cells(self)
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

/// Returns the cell-index groups (one per row, one per column) of cage cells that share a
/// row or column with at least one other cell. Singleton groups are dropped because the
/// all-different check is trivially true on a single cell.
fn row_and_column_groups(cells: &[Cell]) -> Vec<Vec<usize>> {
    let mut by_row: HashMap<Index, Vec<usize>> = HashMap::new();
    let mut by_col: HashMap<Index, Vec<usize>> = HashMap::new();
    for (i, cell) in cells.iter().enumerate() {
        by_row.entry(cell.row).or_default().push(i);
        by_col.entry(cell.column).or_default().push(i);
    }
    by_row
        .into_values()
        .chain(by_col.into_values())
        .filter(|g| g.len() > 1)
        .collect()
}

/// Returns true iff the values at positions `indices` of `tuple` are all distinct.
fn values_all_different(tuple: &[N], indices: &[usize]) -> bool {
    let mut seen = Values::default();
    for &i in indices {
        let bit = Values::new([tuple[i]]);
        if !(seen & bit).is_empty() {
            return false;
        }
        seen = seen | bit;
    }
    true
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
/// Stored behind an [`std::sync::Arc`] so that cloning a [`crate::puzzle::Puzzle`] during search shares this allocation
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
        let a = self.cage_filter(grid)?;
        let b = self.all_different_filter(grid)?;
        Ok(a * b)
    }
    fn cage_filter(&self, grid: &Grid) -> Result<ValueFilter, Error> {
        let mut filter = ValueFilter::default();
        for cage in self.cage.iter() {
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

/// The set of [`Cage`] constraints for a puzzle, backed by a [`Tiling`] that tracks cell
/// coverage and a `HashMap` from polyomino to cage data.
///
/// A cell may belong to at most one cage. Conflict detection on insert scans the new cage's
/// cells against the tiling, which is acceptable because cages are only added at construction
/// time.
#[must_use]
#[derive(Debug, Clone)]
pub struct Cages {
    tiling: Tiling,
    data: HashMap<Polyomino, Cage>,
}

impl Cages {
    /// Creates an empty `Cages` for an `n`×`n` grid.
    pub fn empty(n: Index) -> Self {
        Self {
            tiling: Tiling::empty(n),
            data: HashMap::new(),
        }
    }

    /// Returns a new cage set with the cage inserted.
    ///
    /// Idempotent: if the exact same cage (by polyomino) is already present, returns unchanged.
    /// # Errors
    /// Returns `Error` if any cell in the cage is already claimed by a *different* cage.
    pub fn insert(mut self, cage: Cage) -> Result<Self, Error> {
        if self.data.contains_key(cage.polyomino()) {
            return Ok(self);
        }
        for cell in cage.cells() {
            if let Some(existing_poly) = self.tiling.find_cell(*cell) {
                debug_assert!(
                    self.data.contains_key(existing_poly),
                    "Cages tiling and data are out of sync: {existing_poly:?} in tiling but not in data",
                );
                let existing = self.data[existing_poly].clone();
                return Err(Error::CageConflict(Box::new(cage), Box::new(existing)));
            }
        }
        self.tiling.insert(cage.polyomino().clone());
        self.data.insert(cage.polyomino().clone(), cage);
        Ok(self)
    }

    /// Returns a new cage set with the cage removed.
    ///
    /// Idempotent: if no such cage exists, returns unchanged.
    pub fn remove(mut self, polyomino: &Polyomino) -> Self {
        self.tiling.remove(polyomino);
        self.data.remove(polyomino);
        self
    }

    #[must_use]
    pub fn get(&self, polyomino: &Polyomino) -> Option<&Cage> {
        self.data.get(polyomino)
    }

    #[must_use]
    pub fn contains(&self, polyomino: &Polyomino) -> bool {
        self.data.contains_key(polyomino)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Iterates over every cage in this set.
    pub fn iter(&self) -> impl Iterator<Item = &Cage> + '_ {
        self.data.values()
    }

    /// Returns the cage covering `cell`, or `None` if no cage covers it.
    #[must_use]
    pub fn get_at(&self, cell: Cell) -> Option<&Cage> {
        self.tiling.find_cell(cell).and_then(|p| self.data.get(p))
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

    fn make_cage(cells: &[(Index, Index)], n: N, op: Operation) -> Cage {
        let cells: Vec<Cell> = cells.iter().map(|&(r, c)| Cell::new(r, c)).collect();
        Cage::new(n, Polyomino::new(&cells), op)
    }

    #[test]
    fn cage_operation_returns_stored_operation() {
        let cage = make_cage(&[(0, 0), (0, 1)], 4, Operation::Add(5));
        assert_eq!(cage.operation(), Operation::Add(5));
    }

    #[test]
    fn cage_tuples_match_constructor_output() {
        let cage = make_cage(&[(0, 0)], 4, Operation::Given(3));
        let tuples = cage.tuples();
        assert_eq!(tuples, &[vec![3u8]]);
    }

    #[test]
    fn cage_new_prunes_l_shape_add_six() {
        // L-shape (0,0),(1,0),(1,1) on n=4: column 0 holds positions 0 & 1,
        // row 1 holds positions 1 & 2. Of the 10 raw tuples (3 from {1,1,4},
        // 6 from {1,2,3}, 1 from {2,2,2}), only the 7 that obey both
        // all-different constraints survive.
        let cage = make_cage(&[(0, 0), (1, 0), (1, 1)], 4, Operation::Add(6));
        let tuples: Vec<Vec<N>> = cage.tuples().to_vec();
        assert_eq!(tuples.len(), 7, "got tuples {tuples:?}");
        for t in &tuples {
            assert_ne!(t[0], t[1], "column-0 duplicate in tuple {t:?}");
            assert_ne!(t[1], t[2], "row-1 duplicate in tuple {t:?}");
        }
        assert!(tuples.contains(&vec![1, 4, 1]));
        assert!(!tuples.contains(&vec![1, 1, 4]));
        assert!(!tuples.contains(&vec![4, 1, 1]));
        assert!(!tuples.contains(&vec![2, 2, 2]));
    }

    #[test]
    fn cage_new_prunes_horizontal_pair_add_six() {
        // (0,0),(0,1) with Add(6) on n=4. Raw cage_tuples returns
        // [[2,4],[3,3],[4,2]]; the row-shared all-different constraint drops
        // [3,3].
        let cage = make_cage(&[(0, 0), (0, 1)], 4, Operation::Add(6));
        let mut tuples: Vec<Vec<N>> = cage.tuples().to_vec();
        tuples.sort();
        assert_eq!(tuples, vec![vec![2, 4], vec![4, 2]]);
    }

    #[test]
    fn cage_new_leaves_unique_value_tuples_unchanged() {
        // (0,0),(0,1) with Add(3) on n=4: raw tuples are already all-different,
        // so pruning must be a no-op.
        let cage = make_cage(&[(0, 0), (0, 1)], 4, Operation::Add(3));
        let mut tuples: Vec<Vec<N>> = cage.tuples().to_vec();
        tuples.sort();
        assert_eq!(tuples, vec![vec![1, 2], vec![2, 1]]);
    }

    #[test]
    fn cage_new_singleton_unchanged() {
        let cage = make_cage(&[(0, 0)], 4, Operation::Given(3));
        assert_eq!(cage.tuples(), &[vec![3u8]]);
    }

    #[test]
    fn cage_cells_inherent_matches_polyomino_slice() {
        let cage = make_cage(&[(0, 0), (1, 0)], 4, Operation::Add(5));
        assert_eq!(cage.cells(), cage.polyomino().as_slice());
    }

    #[test]
    fn cages_iter_yields_every_cage() {
        let a = make_cage(&[(0, 0), (0, 1)], 4, Operation::Add(5));
        let b = make_cage(&[(1, 0), (1, 1)], 4, Operation::Add(5));
        let cages = Cages::empty(4).insert(a).unwrap().insert(b).unwrap();
        assert_eq!(cages.iter().count(), 2);
    }

    #[test]
    fn cages_get_at_returns_covering_cage() {
        let a = make_cage(&[(0, 0), (0, 1)], 4, Operation::Add(5));
        let cages = Cages::empty(4).insert(a).unwrap();
        let got = cages.get_at(cell(0, 1)).unwrap();
        assert!(got.cells().contains(&cell(0, 1)));
    }

    #[test]
    fn cages_get_at_returns_none_for_uncovered_cell() {
        let cages = Cages::empty(4);
        assert!(cages.get_at(cell(0, 0)).is_none());
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
