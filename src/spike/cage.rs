//! The spike's cage constraint definition and its tuple-GAC propagator.
//!
//! [`CageDef`] is the structural fix for the historical `Cage.tuples` smell: it
//! describes the constraint (cells, operation, grid size) and **nothing more**.
//! It carries no mutable tuple state. Viable tuples are obtained exclusively
//! through the pure, memoized [`viable_tuples`](crate::spike::cache::viable_tuples).

use crate::{
    Cell, Fill, Operation, Polyomino, Tuple,
    spike::{
        cache::viable_tuples,
        constraint::{Constraint, Outcome, PropagationCtx},
        store::Narrowed,
        variable::Variable,
    },
    types::N,
};

/// An immutable cage constraint definition.
///
/// `cells` is stored as a `Vec` only so [`Constraint::variables`] can hand out a
/// slice; it is the constraint's *scope*, not derived tuple state. The polyomino
/// is retained for tuple generation and collinearity filtering.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CageDef {
    cells: Vec<Cell>,
    polyomino: Polyomino,
    operation: Operation,
    n: N,
}

impl CageDef {
    pub fn new(n: N, polyomino: Polyomino, operation: Operation) -> Self {
        let cells = polyomino.cells().collect();
        Self {
            cells,
            polyomino,
            operation,
            n,
        }
    }

    /// The cage's cells, in row-major order.
    pub fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        self.cells.iter().copied()
    }

    /// All ordered tuples that satisfy the operation, *before* any
    /// store-dependent pruning. A pure function of the cage definition; reused
    /// by [`viable_tuples`](crate::spike::cache::viable_tuples).
    pub fn static_tuples(&self) -> Vec<Tuple> {
        self.polyomino.valid_tuples(self.n, self.operation)
    }
}

impl Constraint<Cell> for CageDef {
    fn variables(&self) -> &[Cell] {
        &self.cells
    }

    /// Tuple-based generalized arc consistency. Reads the viable tuple set
    /// (pure, cached), unions the supported value at each cell position, and
    /// narrows each cell's domain to that union. Writes only domain reductions
    /// to the store; never writes tuple state anywhere.
    fn propagate(&self, ctx: &mut PropagationCtx<Cell>) -> Outcome {
        // Scope the cache borrow: the viable set borrows the cache, so collect
        // the per-cell supported unions before touching the store.
        let unions = {
            let viable = viable_tuples(self, ctx.store, ctx.cache);
            let mut unions = vec![Fill::default(); self.cells.len()];
            for tuple in viable {
                for (slot, &value) in unions.iter_mut().zip(tuple) {
                    *slot = *slot | Fill::new([value]);
                }
            }
            unions
        };
        let mut outcome = Outcome::Unchanged;
        for (cell, union) in self.cells.iter().zip(unions) {
            match ctx.store.intersect(cell.id(), union) {
                Narrowed::Empty => return Outcome::Contradiction,
                Narrowed::Changed => outcome = Outcome::Changed,
                Narrowed::Unchanged => {}
            }
        }
        outcome
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::spike::cache::Cache;

    fn pair() -> Polyomino {
        Polyomino::from_cells(&[Cell::new(0, 0), Cell::new(0, 1)]).unwrap()
    }

    #[test]
    fn variables_are_the_cage_cells() {
        let cage = CageDef::new(4, pair(), Operation::Add(3));
        assert_eq!(cage.variables(), &[Cell::new(0, 0), Cell::new(0, 1)]);
    }

    #[test]
    fn static_tuples_match_polyomino_generation() {
        let cage = CageDef::new(4, pair(), Operation::Add(3));
        let mut got = cage.static_tuples();
        got.sort_unstable();
        // Add(3) on a same-row pair: {1,2} in both orders.
        assert_eq!(got, vec![vec![1u8, 2], vec![2, 1]]);
    }

    #[test]
    fn propagate_narrows_domains_to_supported_values() {
        use crate::spike::store::Store;
        let cage = CageDef::new(4, pair(), Operation::Add(3));
        let mut store = Store::full(4);
        let mut cache = Cache::default();
        let outcome = {
            let mut ctx = PropagationCtx::new(&mut store, &mut cache);
            cage.propagate(&mut ctx)
        };
        assert_eq!(outcome, Outcome::Changed);
        // Only {1,2} support Add(3), so each cell narrows to {1,2}.
        assert_eq!(store.get(Cell::new(0, 0).id()), Fill::new([1, 2]));
        assert_eq!(store.get(Cell::new(0, 1).id()), Fill::new([1, 2]));
    }

    #[test]
    fn propagate_detects_contradiction() {
        use crate::spike::store::Store;
        let cage = CageDef::new(4, pair(), Operation::Add(3));
        let mut store = Store::full(4);
        // Pin both cells to 4: no Add(3) tuple is supported.
        store.set(Cell::new(0, 0).id(), Fill::new([4]));
        store.set(Cell::new(0, 1).id(), Fill::new([4]));
        let mut cache = Cache::default();
        let mut ctx = PropagationCtx::new(&mut store, &mut cache);
        assert_eq!(cage.propagate(&mut ctx), Outcome::Contradiction);
    }
}
