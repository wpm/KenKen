//! The **store**: intrinsic variable domains, keyed by [`VarId`].
//!
//! This is the canonical state — "the puzzle at this moment." It is what gets
//! snapshotted (cloning is a single boxed-slice copy, cheap enough for the
//! eventual search-tree DAG) and what the cache's keys project over. Anything
//! that can be recomputed from the store is *cache*, not store.

use crate::{Cell, Fill, spike::variable::VarId};

/// A variable's domain: the set of values it can still take.
pub type Domain = Fill;

/// Intrinsic state: one [`Domain`] per cell of an `n`×`n` grid, in row-major
/// order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    n: usize,
    domains: Box<[Domain]>,
}

/// The effect of narrowing a domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Narrowed {
    /// The domain was already a subset of the incoming set; nothing changed.
    Unchanged,
    /// The domain shrank but is still non-empty.
    Changed,
    /// The domain became empty — a contradiction.
    Empty,
}

impl Store {
    /// A store where every cell holds the full domain `{1, ..., n}`.
    pub fn full(n: usize) -> Self {
        Self {
            n,
            domains: vec![Fill::full(n); n * n].into_boxed_slice(),
        }
    }

    const fn index(&self, cell: Cell) -> Option<usize> {
        if cell.row < self.n && cell.column < self.n {
            Some(cell.row * self.n + cell.column)
        } else {
            None
        }
    }

    /// The current domain of `id`. An out-of-range id yields the empty domain.
    pub fn get(&self, id: VarId) -> Domain {
        self.index(id.0)
            .map_or_else(Fill::default, |i| self.domains[i])
    }

    /// Replaces the domain of `id`. An out-of-range id is a no-op.
    pub fn set(&mut self, id: VarId, domain: Domain) {
        if let Some(i) = self.index(id.0) {
            self.domains[i] = domain;
        }
    }

    /// Intersects the domain of `id` with `domain`, reporting whether it
    /// changed or emptied. An out-of-range id is a no-op ([`Narrowed::Unchanged`]).
    pub fn intersect(&mut self, id: VarId, domain: Domain) -> Narrowed {
        let Some(i) = self.index(id.0) else {
            return Narrowed::Unchanged;
        };
        let next = self.domains[i] & domain;
        if next == self.domains[i] {
            Narrowed::Unchanged
        } else {
            self.domains[i] = next;
            if next.is_empty() {
                Narrowed::Empty
            } else {
                Narrowed::Changed
            }
        }
    }

    /// The grid size `n`.
    pub const fn n(&self) -> usize {
        self.n
    }

    /// Every cell of the grid in row-major order.
    pub fn cells(&self) -> impl Iterator<Item = Cell> {
        let n = self.n;
        (0..n).flat_map(move |row| (0..n).map(move |column| Cell::new(row, column)))
    }

    /// Every domain is a singleton — the store represents a full assignment.
    pub fn is_solved(&self) -> bool {
        self.domains.iter().all(|d| d.is_singleton())
    }

    /// Some domain is empty — the store is contradictory.
    pub fn is_invalid(&self) -> bool {
        self.domains.iter().any(|d| d.is_empty())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spike::variable::Variable;

    fn vid(row: usize, column: usize) -> VarId {
        Cell::new(row, column).id()
    }

    #[test]
    fn full_store_has_full_domains() {
        let store = Store::full(4);
        assert_eq!(store.n(), 4);
        assert_eq!(store.get(vid(0, 0)), Fill::full(4));
        assert_eq!(store.cells().count(), 16);
    }

    #[test]
    fn set_replaces_domain() {
        let mut store = Store::full(4);
        store.set(vid(1, 2), Fill::new([3]));
        assert_eq!(store.get(vid(1, 2)), Fill::new([3]));
    }

    #[test]
    fn intersect_reports_unchanged_when_superset() {
        let mut store = Store::full(4);
        assert_eq!(
            store.intersect(vid(0, 0), Fill::full(4)),
            Narrowed::Unchanged
        );
    }

    #[test]
    fn intersect_reports_changed_when_narrowed() {
        let mut store = Store::full(4);
        assert_eq!(
            store.intersect(vid(0, 0), Fill::new([1, 2])),
            Narrowed::Changed
        );
        assert_eq!(store.get(vid(0, 0)), Fill::new([1, 2]));
    }

    #[test]
    fn intersect_reports_empty_on_disjoint() {
        let mut store = Store::full(4);
        store.set(vid(0, 0), Fill::new([1]));
        assert_eq!(store.intersect(vid(0, 0), Fill::new([2])), Narrowed::Empty);
        assert!(store.is_invalid());
    }

    #[test]
    fn out_of_range_access_is_inert() {
        let mut store = Store::full(2);
        let outside = vid(5, 5);
        assert_eq!(store.get(outside), Fill::default());
        store.set(outside, Fill::new([1]));
        assert_eq!(store.get(outside), Fill::default());
        assert_eq!(
            store.intersect(outside, Fill::new([1])),
            Narrowed::Unchanged
        );
    }

    #[test]
    fn is_solved_only_when_all_singletons() {
        let store = Store::full(1);
        assert!(store.is_solved());
        let mut store4 = Store::full(4);
        assert!(!store4.is_solved());
        for row in 0..4 {
            for column in 0..4 {
                store4.set(Cell::new(row, column).id(), Fill::new([1]));
            }
        }
        assert!(store4.is_solved());
    }
}
