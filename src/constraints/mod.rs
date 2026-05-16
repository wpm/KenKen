use crate::{Cell, Error, Grid};

pub mod all_different;
pub mod cage;
pub mod polyomino;
pub mod regin;

pub trait Constraint {
    /// Applies this constraint to `grid` and returns the narrowed grid.
    ///
    /// # Errors
    /// Returns [`Error`] if a constraint references a cell outside the grid.
    fn apply_to(&self, grid: &Grid) -> Result<Grid, Error>;
}

/// A set of [`Cell`]s.
pub trait Cover {
    /// The covering [`Cell`]s in row-major order.
    ///
    /// Implementations must be cheap to call repeatedly; the default `len` and
    /// `is_empty` methods each invoke `cells()` independently.
    fn cells(&self) -> impl Iterator<Item = Cell>;

    fn len(&self) -> usize {
        self.cells().count()
    }

    fn is_empty(&self) -> bool {
        self.cells().next().is_none()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    clippy::double_must_use
)]
pub mod test_utils {
    use crate::{Cell, constraints::polyomino::Polyomino};

    /// Test fixture that creates [`Cell`]s from `(row, col)` pairs.
    #[must_use]
    pub fn cells(positions: &[(usize, usize)]) -> Vec<Cell> {
        positions.iter().map(|&(r, c)| Cell::new(r, c)).collect()
    }

    #[must_use]
    pub fn c00() -> Cell {
        Cell::new(0, 0)
    }
    #[must_use]
    pub fn c01() -> Cell {
        Cell::new(0, 1)
    }
    #[must_use]
    pub fn c02() -> Cell {
        Cell::new(0, 2)
    }
    #[must_use]
    pub fn c10() -> Cell {
        Cell::new(1, 0)
    }
    #[must_use]
    pub fn c11() -> Cell {
        Cell::new(1, 1)
    }

    #[must_use]
    pub fn singleton() -> Polyomino {
        Polyomino::from_cells(&cells(&[(0, 0)])).unwrap()
    }

    #[must_use]
    pub fn pair() -> Polyomino {
        Polyomino::from_cells(&cells(&[(0, 0), (0, 1)])).unwrap()
    }

    #[must_use]
    pub fn col_pair() -> Polyomino {
        Polyomino::from_cells(&cells(&[(0, 0), (1, 0)])).unwrap()
    }

    #[must_use]
    pub fn row3() -> Polyomino {
        Polyomino::from_cells(&cells(&[(0, 0), (0, 1), (0, 2)])).unwrap()
    }

    #[must_use]
    pub fn l_shape() -> Polyomino {
        Polyomino::from_cells(&cells(&[(0, 0), (1, 0), (1, 1)])).unwrap()
    }
}
