use std::ops::{BitAnd, BitOr};

use crate::Cage;

/// Possible cell value: a number in the range `1..=9`.
pub type N = u8;
/// A cage target (sum, product, difference, ratio, or given value). Wide enough
/// to hold the largest possible product for a single cage.
pub type M = u16;

/// A cell in a KenKen grid, identified by 0-based row and column `Index` values
/// in row-major order.
#[must_use]
#[derive(Ord, Eq, PartialEq, PartialOrd, Debug, Copy, Clone, Hash)]
pub struct Cell {
    /// 0-based row index.
    pub row: Index,
    /// 0-based column index.
    pub column: Index,
}

impl Cell {
    /// Creates a cell at the given `row` and `column`.
    pub const fn new(row: Index, column: Index) -> Self {
        Self { row, column }
    }

    /// The (up to four) edge-connected cells, with no upper bound check.
    /// Cells off the top or left edge are filtered; cells off the bottom or
    /// right are not.
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
pub struct Fill(u16);

impl Fill {
    /// Creates `Fill` from any iterable of numbers in the range `1..=9`.
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
    /// Values are stored in bits 1–9 of a `u16`, so exactly one value set means
    /// exactly one bit is set, which is equivalent to the inner integer
    /// being a power of two.
    #[must_use]
    pub const fn is_singleton(self) -> bool {
        self.0.is_power_of_two()
    }

    /// Returns the number of candidate values in the set.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    /// Returns a new `Fill` with the value `n` removed.
    pub const fn remove(self, n: N) -> Self {
        Self(self.0 & !(1 << n))
    }
}

impl BitAnd for Fill {
    type Output = Self;

    /// Returns the intersection of two sets of values.
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl FromIterator<N> for Fill {
    fn from_iter<I: IntoIterator<Item = N>>(iter: I) -> Self {
        Self::new(iter)
    }
}

impl BitOr for Fill {
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
    CageConflict(Cage, Cage),
    /// A cage passed to a `Puzzle` method is not present in that puzzle (looked
    /// up by polyomino).
    CageNotInPuzzle(Cage),
    /// A tiling operation referenced a cell that no polyomino covers.
    CellNotCovered(Cell),
    /// Removing a cell from a polyomino would leave the remaining cells
    /// disconnected. Raised by `Polyomino::without`.
    WouldDisconnect(Cell),
    /// A target cell is not edge-connected to any cell of the polyomino it was
    /// applied to.
    TargetNotAdjacent,
    /// A cell passed to `Polyomino::extend` is already in the polyomino.
    CellAlreadyInPolyomino(Cell),
    /// A `Polyomino::without` call would remove the polyomino's only remaining
    /// cell.
    RemovalWouldEmptyPolyomino(Cell),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values_is_empty() {
        assert_eq!(Fill::default(), Fill::new([]));
    }

    #[test]
    fn new_contains_one_through_four() {
        assert_eq!(Fill::new(1..=4), Fill::new([1, 2, 3, 4]));
    }

    #[test]
    fn new_single_value() {
        assert_eq!(Fill::new([1]), Fill::new([1]));
    }

    #[test]
    fn new_one_through_nine() {
        assert_eq!(Fill::new(1..=9), Fill::full(9));
    }

    #[test]
    fn full_contains_one_through_n() {
        assert_eq!(Fill::full(4), Fill::new([1, 2, 3, 4]));
    }

    #[test]
    fn bitand_intersection() {
        assert_eq!(
            Fill::new([1, 2, 3]) & Fill::new([2, 3, 4]),
            Fill::new([2, 3])
        );
    }

    #[test]
    fn bitand_disjoint_is_empty() {
        assert_eq!(Fill::new([1, 2]) & Fill::new([3, 4]), Fill::default());
    }

    #[test]
    fn cell_ordering_is_row_major() {
        assert!(Cell::new(0, 1) < Cell::new(1, 0));
    }

    #[test]
    fn is_singleton_true_for_single_value() {
        assert!(Fill::new([1]).is_singleton());
        assert!(Fill::new([5]).is_singleton());
        assert!(Fill::new([9]).is_singleton());
    }

    #[test]
    fn is_singleton_false_for_empty() {
        assert!(!Fill::default().is_singleton());
    }

    #[test]
    fn is_singleton_false_for_multiple_values() {
        assert!(!Fill::new([1, 2]).is_singleton());
        assert!(!Fill::full(4).is_singleton());
    }

    #[test]
    fn is_empty_true_for_default() {
        assert!(Fill::default().is_empty());
    }

    #[test]
    fn is_empty_false_for_non_empty() {
        assert!(!Fill::new([1]).is_empty());
        assert!(!Fill::full(9).is_empty());
    }

    #[test]
    fn len_matches_number_of_values() {
        assert_eq!(Fill::default().len(), 0);
        assert_eq!(Fill::new([3]).len(), 1);
        assert_eq!(Fill::new([1, 5, 9]).len(), 3);
        assert_eq!(Fill::full(9).len(), 9);
    }

    #[test]
    fn remove_drops_a_present_value() {
        assert_eq!(Fill::new([1, 2, 3]).remove(2), Fill::new([1, 3]));
    }

    #[test]
    fn remove_absent_value_is_noop() {
        assert_eq!(Fill::new([1, 3]).remove(2), Fill::new([1, 3]));
    }

    #[test]
    fn bitor_union() {
        assert_eq!(Fill::new([1, 2]) | Fill::new([2, 3]), Fill::new([1, 2, 3]));
    }

    #[test]
    fn bitor_disjoint() {
        assert_eq!(
            Fill::new([1, 2]) | Fill::new([3, 4]),
            Fill::new([1, 2, 3, 4])
        );
    }

    #[test]
    fn from_iterator_collects_values() {
        let v: Fill = [1u8, 2, 3].into_iter().collect();
        assert_eq!(v, Fill::new([1, 2, 3]));
    }

    #[test]
    fn neighbors_4_interior_yields_four() {
        let n: Vec<Cell> = Cell::new(2, 2).neighbors_4().collect();
        assert_eq!(n.len(), 4);
        assert!(n.contains(&Cell::new(1, 2)));
        assert!(n.contains(&Cell::new(3, 2)));
        assert!(n.contains(&Cell::new(2, 1)));
        assert!(n.contains(&Cell::new(2, 3)));
    }

    #[test]
    fn neighbors_4_top_left_corner_yields_two() {
        let n: Vec<Cell> = Cell::new(0, 0).neighbors_4().collect();
        assert_eq!(n.len(), 2);
        assert!(n.contains(&Cell::new(1, 0)));
        assert!(n.contains(&Cell::new(0, 1)));
    }
}
