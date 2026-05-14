use crate::{
    Cell, Fill,
    constraints::{Constraint, RegionConstraint, cover::Cover, regin::regin},
    grid::Grid,
};

/// A constraint that ensures each cell in a row or column has a different
/// value.
#[must_use]
#[derive(Clone)]
pub struct AllDifferent(Vec<Cell>);

impl AllDifferent {
    /// A row of [`Cell`]s that must all have a different value.
    /// # Panics
    /// Panics if `n` is zero.
    pub fn row(n: usize, index: usize) -> Self {
        assert!(n > 0);
        Self((0..n).map(|column| Cell::new(index, column)).collect())
    }

    /// A column of [`Cell`]s that must all have a different value.
    /// # Panics
    /// Panics if `n` is zero.
    pub fn column(n: usize, index: usize) -> Self {
        assert!(n > 0);
        Self((0..n).map(|row| Cell::new(row, index)).collect())
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
mod tests {
    // --- Region::row ---

    use crate::{
        Cell,
        constraints::{all_different::AllDifferent, cover::Cover},
    };

    #[test]
    fn row_contains_correct_cells() {
        let r = AllDifferent::row(4, 2);
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

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn row_panics_on_zero_n() {
        let _ = AllDifferent::row(0, 0);
    }

    // --- Region::column ---

    #[test]
    fn column_contains_correct_cells() {
        let r = AllDifferent::column(3, 1);
        assert_eq!(
            r.cells(),
            vec![Cell::new(0, 1), Cell::new(1, 1), Cell::new(2, 1)]
        );
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn column_panics_on_zero_n() {
        let _ = AllDifferent::column(0, 0);
    }

    #[test]
    fn len_equals_n() {
        assert_eq!(AllDifferent::row(4, 0).len(), 4);
        assert_eq!(AllDifferent::column(3, 1).len(), 3);
    }
}
