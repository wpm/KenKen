//! The **cache**: derived viable-tuple sets, memoized.
//!
//! [`viable_tuples`] is a **pure function** of `(cage, projected domains)`. The
//! cache exists for performance only and is never the source of truth:
//!
//! - Each entry memoizes the pure computation `compute_viable_tuples`.
//! - The key projects onto only the cage's cells' current domains, so an entry is reusable across
//!   different stores that agree on that subset (and so naturally shared across search-tree nodes
//!   that agree there).
//! - Clearing the cache ([`Cache::clear`]) can only change performance, never observable behavior —
//!   verified by the tests below.

use std::collections::HashMap;

use crate::{
    Fill, Tuple,
    spike::{cage::CageDef, store::Store, variable::Variable},
    types::N,
};

/// A set of viable ordered tuples for a cage.
pub type TupleSet = Vec<Tuple>;

/// Memo key: the cage plus its cells' current domains, projected to value lists
/// so two stores agreeing on the cage's cells hit the same entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TupleKey {
    cage: CageDef,
    projection: Vec<Vec<N>>,
}

/// Derived state: a memo of [`viable_tuples`] results. Populated lazily.
#[derive(Debug, Default)]
pub struct Cache {
    tuples: HashMap<TupleKey, TupleSet>,
}

impl Cache {
    /// Drops all memoized entries. Affects performance only.
    pub fn clear(&mut self) {
        self.tuples.clear();
    }

    #[cfg(test)]
    pub fn entry_count(&self) -> usize {
        self.tuples.len()
    }
}

fn projection(cage: &CageDef, store: &Store) -> Vec<Vec<N>> {
    cage.cells()
        .map(|cell| store.get(cell.id()).iter().collect())
        .collect()
}

/// The pure computation behind the cache: the cage's static tuples filtered to
/// those consistent with the current domains of the cage's cells.
fn compute_viable_tuples(cage: &CageDef, store: &Store) -> TupleSet {
    let domains: Vec<Fill> = cage.cells().map(|cell| store.get(cell.id())).collect();
    cage.static_tuples()
        .into_iter()
        .filter(|tuple| {
            tuple
                .iter()
                .zip(&domains)
                .all(|(&value, domain)| !(*domain & Fill::new([value])).is_empty())
        })
        .collect()
}

/// Returns the viable tuples for `cage` under `store`, memoized in `cache`.
///
/// Pure: for the same cage and the same store contents over the cage's cells it
/// always returns the same set. The cache is a transparent memo of
/// `compute_viable_tuples`.
pub fn viable_tuples<'c>(cage: &CageDef, store: &Store, cache: &'c mut Cache) -> &'c TupleSet {
    let key = TupleKey {
        cage: cage.clone(),
        projection: projection(cage, store),
    };
    cache
        .tuples
        .entry(key)
        .or_insert_with(|| compute_viable_tuples(cage, store))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Cell, Operation, Polyomino, spike::store::Store};

    fn cage() -> CageDef {
        let poly = Polyomino::from_cells(&[Cell::new(0, 0), Cell::new(0, 1)]).unwrap();
        CageDef::new(4, poly, Operation::Add(4))
    }

    #[test]
    fn viable_tuples_filters_by_current_domains() {
        let cage = cage();
        let mut store = Store::full(4);
        // Pin (0,0) to {1}: only the tuple [1,3] survives for Add(4) on a row pair
        // ([2,2] is filtered by collinearity, [3,1] needs (0,0)=3).
        store.set(Cell::new(0, 0).id(), Fill::new([1]));
        let mut cache = Cache::default();
        let viable = viable_tuples(&cage, &store, &mut cache).clone();
        assert_eq!(viable, vec![vec![1u8, 3]]);
    }

    #[test]
    fn cache_is_pure_clearing_does_not_change_result() {
        let cage = cage();
        let store = Store::full(4);
        let mut cache = Cache::default();

        let cold = viable_tuples(&cage, &store, &mut cache).clone();
        assert_eq!(cache.entry_count(), 1);

        // A second call with the same inputs is a cache hit: same result.
        let warm = viable_tuples(&cage, &store, &mut cache).clone();
        assert_eq!(cache.entry_count(), 1);
        assert_eq!(cold, warm);

        // Clearing and recomputing yields the identical result — the cache
        // affects only performance, never the value.
        cache.clear();
        assert_eq!(cache.entry_count(), 0);
        let recomputed = viable_tuples(&cage, &store, &mut cache).clone();
        assert_eq!(cold, recomputed);

        // And the cached value equals a fresh, uncached pure computation.
        assert_eq!(cold, compute_viable_tuples(&cage, &store));
    }

    #[test]
    fn distinct_projections_get_distinct_entries() {
        let cage = cage();
        let mut cache = Cache::default();

        let full = Store::full(4);
        let _ = viable_tuples(&cage, &full, &mut cache);

        let mut narrowed = Store::full(4);
        narrowed.set(Cell::new(0, 0).id(), Fill::new([1]));
        let _ = viable_tuples(&cage, &narrowed, &mut cache);

        assert_eq!(cache.entry_count(), 2);
    }
}
