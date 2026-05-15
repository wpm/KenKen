use crate::{
    Cell, Error, Fill,
    constraints::{Constraint, RegionConstraint, cover::Cover, regin::regin},
    grid::Grid,
};

/// A constraint that ensures each cell in a row or column has a different
/// value.
#[must_use]
#[derive(Debug, Clone)]
pub struct AllDifferent(Vec<Cell>);

impl AllDifferent {
    /// A row of [`Cell`]s that must all have a different value.
    /// # Errors
    /// Returns [`Error::IndexOutOfRange`] if `index` is not less than `n`.
    pub fn row(n: usize, index: usize) -> Result<Self, Error> {
        if index >= n {
            return Err(Error::IndexOutOfRange(index, n));
        }
        Ok(Self(
            (0..n).map(|column| Cell::new(index, column)).collect(),
        ))
    }

    /// A column of [`Cell`]s that must all have a different value.
    /// # Errors
    /// Returns [`Error::IndexOutOfRange`] if `index` is not less than `n`.
    pub fn column(n: usize, index: usize) -> Result<Self, Error> {
        if index >= n {
            return Err(Error::IndexOutOfRange(index, n));
        }
        Ok(Self((0..n).map(|row| Cell::new(row, index)).collect()))
    }
}

impl Cover for AllDifferent {
    fn cells(&self) -> Vec<Cell> {
        self.0.clone()
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

impl RegionConstraint for AllDifferent {
    fn constraint(&self, grid: &Grid) -> Constraint {
        let cells = self.cells();
        #[allow(clippy::expect_used)]
        let grid_values: Vec<Fill> = cells
            .iter()
            .map(|c| grid.get(c).expect("AllDifferent cell outside grid"))
            .collect();
        let all_different_values = regin(&grid_values);
        cells.iter().copied().zip(all_different_values).collect()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    // --- Region::row ---

    use crate::{
        Cell, Error,
        constraints::{all_different::AllDifferent, cover::Cover},
    };

    #[test]
    fn row_contains_correct_cells() {
        let r = AllDifferent::row(4, 2).unwrap();
        assert_eq!(
            r.cells(),
            vec![
                Cell::new(2, 0),
                Cell::new(2, 1),
                Cell::new(2, 2),
                Cell::new(2, 3)
            ]
        );
    }

    fn assert_index_out_of_range(f: impl Fn(usize, usize) -> Result<AllDifferent, Error>) {
        assert!(matches!(f(0, 0), Err(Error::IndexOutOfRange(0, 0))));
        assert!(matches!(f(3, 3), Err(Error::IndexOutOfRange(3, 3))));
        assert!(matches!(f(3, 5), Err(Error::IndexOutOfRange(5, 3))));
    }

    #[test]
    fn row_index_out_of_range_returns_err() {
        assert_index_out_of_range(AllDifferent::row);
    }

    // --- Region::column ---

    #[test]
    fn column_contains_correct_cells() {
        let r = AllDifferent::column(3, 1).unwrap();
        assert_eq!(
            r.cells(),
            vec![Cell::new(0, 1), Cell::new(1, 1), Cell::new(2, 1)]
        );
    }

    #[test]
    fn column_index_out_of_range_returns_err() {
        assert_index_out_of_range(AllDifferent::column);
    }

    #[test]
    fn len_equals_n() {
        assert_eq!(AllDifferent::row(4, 0).unwrap().len(), 4);
        assert_eq!(AllDifferent::column(3, 1).unwrap().len(), 3);
    }
}
