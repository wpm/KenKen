//! The **cache**: derived viable-tuple sets, memoized.
//!
//! [`viable_tuples`] is a **pure function** of `(cage, projected domains)`. The
//! cache exists for performance only and is never the source of truth:
//!
//! - Static tuples are memoized per cage; viable sets are memoized per `(cage, projected domains)`.
//! - The key projects onto only the cage's cells' current domains, so an entry is reusable across
//!   stores that agree on that subset (and so naturally shared across search-tree nodes that agree
//!   there).
//! - It is never the source of truth: an independent empty cache yields the identical result, so
//!   the cache only changes performance, never observable behavior.

use std::{collections::HashMap, rc::Rc};

use crate::{Fill, Tuple, cage::Cage, store::Store, types::N, variable::Variable};

/// A set of viable ordered tuples for a cage.
pub type TupleSet = Vec<Tuple>;

/// Memo key: a cage plus its cells' current domains, projected to value lists so
/// two stores agreeing on the cage's cells hit the same entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ViableKey {
    cage: Cage,
    projection: Vec<Vec<N>>,
}

/// Derived state, populated lazily. Two memos: a cage's full static tuple set
/// (independent of the store) and the viable subset under a given projection.
#[derive(Debug, Default)]
pub struct Cache {
    statics: HashMap<Cage, Rc<[Tuple]>>,
    viable: HashMap<ViableKey, TupleSet>,
}

impl Cache {
    #[cfg(test)]
    pub fn viable_entry_count(&self) -> usize {
        self.viable.len()
    }

    fn static_tuples(&mut self, cage: &Cage) -> Rc<[Tuple]> {
        if let Some(cached) = self.statics.get(cage) {
            return Rc::clone(cached);
        }
        let computed: Rc<[Tuple]> = Rc::from(cage.tuples());
        let _ = self.statics.insert(cage.clone(), Rc::clone(&computed));
        computed
    }
}

fn projection(cage: &Cage, store: &Store) -> Vec<Vec<N>> {
    cage.cells()
        .map(|cell| store.get(cell.id()).iter().collect())
        .collect()
}

fn filter_viable(statics: &[Tuple], domains: &[Fill]) -> TupleSet {
    statics
        .iter()
        .filter(|tuple| {
            tuple
                .iter()
                .zip(domains)
                .all(|(&value, domain)| !(*domain & Fill::new([value])).is_empty())
        })
        .cloned()
        .collect()
}

/// Returns the viable tuples for `cage` under `store`, memoized in `cache`.
///
/// Pure: for the same cage and the same store contents over the cage's cells it
/// always returns the same set, regardless of cache state.
pub fn viable_tuples<'c>(cage: &Cage, store: &Store, cache: &'c mut Cache) -> &'c TupleSet {
    let key = ViableKey {
        cage: cage.clone(),
        projection: projection(cage, store),
    };
    if !cache.viable.contains_key(&key) {
        let statics = cache.static_tuples(cage);
        let domains: Vec<Fill> = cage.cells().map(|cell| store.get(cell.id())).collect();
        let filtered = filter_viable(&statics, &domains);
        let _ = cache.viable.insert(key.clone(), filtered);
    }
    &cache.viable[&key]
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{Cell, Operation, Polyomino};

    fn cage() -> Cage {
        let poly = Polyomino::from_cells(&[Cell::new(0, 0), Cell::new(0, 1)]).unwrap();
        Cage::new(4, poly, Operation::Add(4))
    }

    #[test]
    fn viable_tuples_filters_by_current_domains() {
        let cage = cage();
        let mut store = Store::full(4);
        // Pin (0,0) to {1}: only [1,3] survives for Add(4) on a row pair.
        store.set(Cell::new(0, 0).id(), Fill::new([1]));
        let mut cache = Cache::default();
        let viable = viable_tuples(&cage, &store, &mut cache).clone();
        assert_eq!(viable, vec![vec![1u8, 3]]);
    }

    #[test]
    fn cache_is_a_pure_memo() {
        let cage = cage();
        let store = Store::full(4);

        // A repeated call on the same cache is a hit and yields the same value.
        let mut cache = Cache::default();
        let first = viable_tuples(&cage, &store, &mut cache).clone();
        let second = viable_tuples(&cage, &store, &mut cache).clone();
        assert_eq!(cache.viable_entry_count(), 1);
        assert_eq!(first, second);

        // An independent, empty cache produces the identical value: the cache
        // affects only performance, never the result.
        let mut fresh = Cache::default();
        assert_eq!(first, *viable_tuples(&cage, &store, &mut fresh));

        // And both equal a fresh, uncached pure computation.
        let domains: Vec<Fill> = cage.cells().map(|c| store.get(c.id())).collect();
        assert_eq!(first, filter_viable(&cage.tuples(), &domains));
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

        assert_eq!(cache.viable_entry_count(), 2);
    }
}
