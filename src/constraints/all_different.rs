use crate::constraints::constraint::{Constraint, ValueFilter};
use crate::constraints::regin::regin;
use crate::geometry::shape::Cells;
use crate::types::Index;
use crate::{Cell, Grid, Values};

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
    fn value_filter(&self, grid: &Grid) -> ValueFilter {
        let cells = self.cells();
        let grid_values: Vec<Values> = cells.iter().map(|c| grid.get_or_default(c)).collect();
        let all_different_values = regin(&grid_values);
        cells.iter().copied().zip(all_different_values).collect()
    }
}

impl Cells for AllDifferent {
    fn cells(&self) -> &[Cell] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::Cell;
    use crate::constraints::all_different::AllDifferent;
    use crate::geometry::shape::Cells;

    #[test]
    fn line_row_has_correct_cells() {
        let row = AllDifferent::row(4, 2);
        for c in 0..4 {
            assert!(row.cells().contains(&Cell::new(2, c)));
        }
    }

    #[test]
    fn line_column_has_correct_cells() {
        let column = AllDifferent::column(4, 1);
        for r in 0..4 {
            assert!(column.cells().contains(&Cell::new(r, 1)));
        }
    }
}
