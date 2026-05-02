use crate::Cell;

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
}

impl Cells for Polyomino {
    fn cells(&self) -> Vec<Cell> {
        self.0.clone()
    }
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
}
