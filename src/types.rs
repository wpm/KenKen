#![allow(dead_code)]

use std::ops::{BitAnd, BitOr};

/// Possible cell value: a number in the range `1..=9`.
pub type N = u8;
/// Possible product of cell values: a number in the range `1..=9 * 9 * 9`.
pub type M = u16;

/// A cell in a KenKen grid, identified by 0-based row and column `Index` values in row-major order.
#[must_use]
#[derive(Ord, Eq, PartialEq, PartialOrd, Debug, Copy, Clone, Hash)]
pub struct Cell {
    pub row: Index,
    pub column: Index,
}

impl Cell {
    pub const fn new(row: Index, column: Index) -> Self {
        Self { row, column }
    }

    /// The (up to four) 4-adjacent cells, with no upper bound check.
    /// Cells off the top or left edge are filtered; cells off the bottom or right are not.
    pub fn neighbors_4(self) -> impl Iterator<Item = Self> {
        [
            self.row.checked_sub(1).map(|r| Self::new(r, self.column)),
            Some(Self::new(self.row + 1, self.column)),
            self.column.checked_sub(1).map(|c| Self::new(self.row, c)),
            Some(Self::new(self.row, self.column + 1)),
        ]
        .into_iter()
        .flatten()
    }
}
/// A 0-based row or column index.
pub type Index = usize;

/// A set of candidate values in `1..=9` for a cell stored as a bitmap.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Values(u16);

impl Values {
    /// Creates a `Values` from any iterable of numbers in the range `1..=9`.
    pub fn new(ns: impl IntoIterator<Item = N>) -> Self {
        Self(
            ns.into_iter()
                .fold(0u16, |acc, n| acc | (1u16 << u32::from(n))),
        )
    }

    /// Returns the full set `{1, ..., n}`.
    #[allow(clippy::cast_possible_truncation)]
    pub fn full(n: Index) -> Self {
        Self::new(1..=(n as N))
    }

    /// Returns an iterator over the values in ascending order.
    pub fn iter(self) -> impl Iterator<Item = N> {
        (1u8..=9).filter(move |&v| self.0 & (1u16 << v) != 0)
    }

    /// Returns true if the set contains no values.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns true if exactly one value is set.
    ///
    /// Values are stored in bits 1–9 of a `u16`, so exactly one value set means exactly one bit
    /// is set, which is equivalent to the inner integer being a power of two.
    #[must_use]
    pub const fn is_singleton(self) -> bool {
        self.0.is_power_of_two()
    }

    /// Returns the number of candidate values in the set.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    /// Returns a new `Values` with `n` removed.
    pub const fn remove(self, n: N) -> Self {
        Self(self.0 & !(1 << n))
    }
}

impl BitAnd for Values {
    type Output = Self;
    /// Returns the intersection of two sets of values.
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl FromIterator<N> for Values {
    fn from_iter<I: IntoIterator<Item = N>>(iter: I) -> Self {
        Self::new(iter)
    }
}

impl BitOr for Values {
    type Output = Self;
    /// Returns the union of two sets of values.
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Errors that can occur during puzzle construction or solving.
#[derive(Debug)]
pub enum Error {
    /// A grid with no cells.
    InvalidGridSize(Index),
    /// A referenced cell is not present in the grid.
    InvalidCell(Cell),
    /// A new cage conflicts with an existing cage: `(new_cage, existing_cage)`.
    CageConflict(Box<crate::constraints::Cage>, Box<crate::constraints::Cage>),
    /// A tiling operation referenced a cell that no polyomino covers.
    CellNotCovered(Cell),
    /// Removing a cell from a polyomino would leave the remaining cells disconnected.
    /// Raised by `Tiling::flip` (when the source polyomino splits) and by
    /// `Polyomino::without`.
    FlipWouldDisconnect(Cell),
    /// A flip target polyomino has no cell 4-adjacent to the cell being flipped.
    TargetNotAdjacent,
    /// Two polyominos passed to `merge_split` are not 4-adjacent.
    PolyominosNotAdjacent,
    /// A cell passed to `Polyomino::extend` is already in the polyomino.
    CellAlreadyInPolyomino(Cell),
    /// A `Polyomino::without` call would remove the polyomino's only remaining cell.
    RemovalWouldEmptyPolyomino(Cell),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values_is_empty() {
        assert_eq!(Values::default(), Values::new([]));
    }

    #[test]
    fn new_contains_one_through_four() {
        assert_eq!(Values::new(1..=4), Values::new([1, 2, 3, 4]));
    }

    #[test]
    fn new_single_value() {
        assert_eq!(Values::new([1]), Values::new([1]));
    }

    #[test]
    fn new_one_through_nine() {
        assert_eq!(Values::new(1..=9), Values::full(9));
    }

    #[test]
    fn full_contains_one_through_n() {
        assert_eq!(Values::full(4), Values::new([1, 2, 3, 4]));
    }

    #[test]
    fn bitand_intersection() {
        assert_eq!(
            Values::new([1, 2, 3]) & Values::new([2, 3, 4]),
            Values::new([2, 3])
        );
    }

    #[test]
    fn bitand_disjoint_is_empty() {
        assert_eq!(Values::new([1, 2]) & Values::new([3, 4]), Values::default());
    }

    #[test]
    fn cell_ordering_is_row_major() {
        assert!(Cell::new(0, 1) < Cell::new(1, 0));
    }

    #[test]
    fn is_singleton_true_for_single_value() {
        assert!(Values::new([1]).is_singleton());
        assert!(Values::new([5]).is_singleton());
        assert!(Values::new([9]).is_singleton());
    }

    #[test]
    fn is_singleton_false_for_empty() {
        assert!(!Values::default().is_singleton());
    }

    #[test]
    fn is_singleton_false_for_multiple_values() {
        assert!(!Values::new([1, 2]).is_singleton());
        assert!(!Values::full(4).is_singleton());
    }
}
