use crate::Cell;
use std::collections::HashSet;

/// All cells in a single row of an `n`×`n` grid.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Row {
    n: usize,
    row: usize,
}

/// All cells in a single column of an `n`×`n` grid.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Column {
    n: usize,
    column: usize,
}

/// An arbitrary set of cells forming a polyomino, stored in sorted order without duplicates.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Polyomino(Vec<Cell>);

/// A shape that covers a set of cells in the grid.
pub trait Cells {
    fn cells(&self) -> Vec<Cell>;
}

impl Cells for Row {
    fn cells(&self) -> Vec<Cell> {
        (0..self.n)
            .map(|column| Cell::new(self.row, column))
            .collect()
    }
}

impl Cells for Column {
    fn cells(&self) -> Vec<Cell> {
        (0..self.n).map(|row| Cell::new(row, self.column)).collect()
    }
}

impl Polyomino {
    /// Creates a `Polyomino` from a slice of cells, sorting and deduplicating them.
    pub fn new(cells: &[Cell]) -> Self {
        let mut cells = cells.to_vec();
        cells.sort();
        cells.dedup();
        Self(cells)
    }

    /// Returns the cells as a slice.
    pub fn as_slice(&self) -> &[Cell] {
        &self.0
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns true if `cell` is one of this polyomino's cells.
    ///
    /// Cells are sorted, so lookup is O(log n).
    #[must_use]
    pub fn contains_cell(&self, cell: Cell) -> bool {
        self.0.binary_search(&cell).is_ok()
    }
}

impl Cells for Polyomino {
    fn cells(&self) -> Vec<Cell> {
        self.0.clone()
    }
}

/// Returns true if `cells` form a single 4-connected component (or is empty).
#[must_use]
pub fn is_connected(cells: &[Cell]) -> bool {
    let cell_set: HashSet<Cell> = cells.iter().copied().collect();
    is_connected_set(&cell_set)
}

/// As [`is_connected`], but takes a pre-built `HashSet` so callers that already have one
/// avoid a second allocation.
#[must_use]
pub fn is_connected_set<S: std::hash::BuildHasher>(cells: &HashSet<Cell, S>) -> bool {
    let Some(&start) = cells.iter().next() else {
        return true;
    };
    let mut visited: HashSet<Cell> = HashSet::with_capacity(cells.len());
    visited.insert(start);
    let mut stack: Vec<Cell> = vec![start];
    while let Some(cell) = stack.pop() {
        for n in cell.neighbors_4() {
            if cells.contains(&n) && visited.insert(n) {
                stack.push(n);
            }
        }
    }
    visited.len() == cells.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_cells_all_in_correct_row() {
        let row = Row { n: 4, row: 2 };
        let expected: Vec<Cell> = (0..4).map(|c| Cell::new(2, c)).collect();
        assert_eq!(row.cells(), expected);
    }

    #[test]
    fn column_cells_all_in_correct_column() {
        let col = Column { n: 4, column: 1 };
        let expected: Vec<Cell> = (0..4).map(|r| Cell::new(r, 1)).collect();
        assert_eq!(col.cells(), expected);
    }

    #[test]
    fn polyomino_cells_are_sorted() {
        let mut cells = vec![Cell::new(2, 1), Cell::new(0, 3), Cell::new(1, 0)];
        let polyomino = Polyomino::new(&cells);
        cells.sort();
        assert_eq!(polyomino.cells(), cells);
    }

    #[test]
    fn cage_cells_deduplicates() {
        let cells = vec![Cell::new(0, 0), Cell::new(0, 0), Cell::new(1, 1)];
        let polyomino = Polyomino::new(&cells);
        assert_eq!(polyomino.cells().len(), 2);
    }

    #[test]
    fn is_connected_empty_is_true() {
        assert!(is_connected(&[]));
    }

    #[test]
    fn is_connected_single_cell_is_true() {
        assert!(is_connected(&[Cell::new(2, 3)]));
    }

    #[test]
    fn is_connected_two_adjacent_cells_is_true() {
        assert!(is_connected(&[Cell::new(0, 0), Cell::new(0, 1)]));
        assert!(is_connected(&[Cell::new(0, 0), Cell::new(1, 0)]));
    }

    #[test]
    fn is_connected_two_non_adjacent_cells_is_false() {
        assert!(!is_connected(&[Cell::new(0, 0), Cell::new(0, 2)]));
        assert!(!is_connected(&[Cell::new(0, 0), Cell::new(2, 0)]));
    }

    #[test]
    fn is_connected_l_shape_is_true() {
        assert!(is_connected(&[
            Cell::new(0, 0),
            Cell::new(1, 0),
            Cell::new(1, 1),
        ]));
    }

    #[test]
    fn is_connected_diagonal_is_false() {
        assert!(!is_connected(&[Cell::new(0, 0), Cell::new(1, 1)]));
    }
}
