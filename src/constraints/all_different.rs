use crate::{
    Cell, Error, Grid,
    constraints::{Constraint, Cover, regin::regin},
};

/// A constraint that ensures every cell in a row or column contains a different
/// value.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AllDifferent {
    cells: Vec<Cell>,
}

impl AllDifferent {
    /// A row of [`Cell`]s on an `n`×`n` grid that must all have a different value.
    /// # Errors
    /// Returns [`Error::IndexOutOfRange`] if `index` is not less than the grid size `n`.
    pub fn row(n: usize, index: usize) -> Result<Self, Error> {
        if index >= n {
            return Err(Error::IndexOutOfRange(index, n));
        }
        let cells = (0..n).map(|column| Cell::new(index, column)).collect();
        Ok(Self { cells })
    }

    /// A column of [`Cell`]s on an `n`×`n` grid that must all have a different value.
    /// # Errors
    /// Returns [`Error::IndexOutOfRange`] if `index` is not less than the grid size `n`.
    pub fn column(n: usize, index: usize) -> Result<Self, Error> {
        if index >= n {
            return Err(Error::IndexOutOfRange(index, n));
        }
        let cells = (0..n).map(|row| Cell::new(row, index)).collect();
        Ok(Self { cells })
    }
}

impl Constraint for AllDifferent {
    fn apply_to(&self, grid: &Grid) -> Result<Grid, Error> {
        let cells: Vec<Cell> = self.cells().collect();
        let fills = cells
            .iter()
            .map(|cell| grid.get(cell))
            .collect::<Result<Vec<_>, _>>()?;
        let fill_constraints = cells.into_iter().zip(regin(&fills)).collect();
        grid.apply(fill_constraints)
    }
}

impl Cover for AllDifferent {
    fn cells(&self) -> impl Iterator<Item = Cell> {
        self.cells.iter().copied()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::{
        Cell, Error,
        constraints::{Cover, all_different::AllDifferent},
    };

    fn row_4() -> AllDifferent {
        AllDifferent::row(4, 2).unwrap()
    }

    fn column_3() -> AllDifferent {
        AllDifferent::column(3, 1).unwrap()
    }

    fn assert_index_out_of_range(f: impl Fn(usize, usize) -> Result<AllDifferent, Error>) {
        assert!(f(0, 0).is_err());
        assert!(matches!(f(3, 3), Err(Error::IndexOutOfRange(3, 3))));
        assert!(matches!(f(3, 5), Err(Error::IndexOutOfRange(5, 3))));
    }

    // --- AllDifferent::row ---

    #[test]
    fn row_contains_correct_cells() {
        assert_eq!(
            row_4().cells().collect::<Vec<_>>(),
            vec![
                Cell::new(2, 0),
                Cell::new(2, 1),
                Cell::new(2, 2),
                Cell::new(2, 3)
            ]
        );
    }

    #[test]
    fn row_index_out_of_range_returns_err() {
        assert_index_out_of_range(AllDifferent::row);
    }

    // --- AllDifferent::column ---

    #[test]
    fn column_contains_correct_cells() {
        assert_eq!(
            column_3().cells().collect::<Vec<_>>(),
            vec![Cell::new(0, 1), Cell::new(1, 1), Cell::new(2, 1)]
        );
    }

    #[test]
    fn column_index_out_of_range_returns_err() {
        assert_index_out_of_range(AllDifferent::column);
    }

    #[test]
    fn len_equals_n() {
        assert_eq!(row_4().len(), 4);
        assert_eq!(column_3().len(), 3);
    }

    #[test]
    fn is_empty_is_false_for_nonempty_constraint() {
        assert!(!row_4().is_empty());
        assert!(!column_3().is_empty());
    }
}
