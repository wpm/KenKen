use std::collections::HashMap;

use crate::{
    Cell, Cover, Error, Fill, Grid, Operation, Polyomino, constraints::Constraint, types::N,
};

pub mod arithmetic;
pub mod operation;

/// An ordered assignment of values to the cells of a cage, one value per cell.
pub type Tuple = Vec<N>;

/// A polyomino-shaped constraint whose cell values satisfy an arithmetic condition.
///
/// Stores the valid ordered tuples for the operation after filtering out
/// assignments that repeat a value within any shared row or column of the
/// polyomino.
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Cage {
    polyomino: Polyomino,
    operation: Operation,
    tuples: Vec<Tuple>,
}

impl Cage {
    /// Creates a cage over the given polyomino. Stores valid ordered tuples for
    /// `operation`, with tuples that repeat a value within any shared row
    /// or column dropped.
    #[must_use]
    pub fn new(n: N, polyomino: Polyomino, operation: Operation) -> Self {
        let tuples = polyomino.valid_tuples(n, operation);
        Self {
            polyomino,
            operation,
            tuples,
        }
    }

    /// Returns the polyomino covered by this cage.
    pub const fn polyomino(&self) -> &Polyomino {
        &self.polyomino
    }

    /// Returns this cage's operation.
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }

    /// Returns a slice over the cage's precomputed valid ordered tuples.
    #[must_use]
    pub fn tuples(&self) -> &[Tuple] {
        &self.tuples
    }
}

impl Constraint for Cage {
    fn apply_to(&self, grid: &Grid) -> Result<Grid, Error> {
        let n = self.len();
        // TODO Rewrite this with fold().
        let mut slots = vec![Fill::default(); n];
        for tuple in self.tuples() {
            for (slot, &val) in slots.iter_mut().zip(tuple.iter()) {
                *slot = *slot | Fill::new([val]);
            }
        }
        let fill_constraints: HashMap<Cell, Fill> = self.cells().zip(slots).collect();
        grid.clone().apply(fill_constraints)
    }
}

impl Cover for Cage {
    fn cells(&self) -> impl Iterator<Item = Cell> {
        self.polyomino.cells()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use crate::{
        Operation,
        constraints::{
            cage::Cage,
            test_utils::{l_shape, pair, singleton},
        },
    };
    // --- Cage::new ---

    #[test]
    fn cage_new_given_singleton() {
        let cage = Cage::new(4, singleton(), Operation::Given(3));
        assert_eq!(cage.tuples(), &[vec![3u8]]);
    }

    #[test]
    fn cage_new_subtract_same_row_pair() {
        // Same-row pair: both orderings still survive since values differ.
        let cage = Cage::new(4, pair(), Operation::Subtract(1));
        let mut tuples = cage.tuples().to_vec();
        tuples.sort_unstable();
        // Subtract(1): multisets [1,2],[2,3],[3,4] → both orderings each.
        assert_eq!(
            tuples,
            vec![
                vec![1u8, 2],
                vec![2, 1],
                vec![2, 3],
                vec![3, 2],
                vec![3, 4],
                vec![4, 3],
            ]
        );
    }

    #[test]
    fn cage_new_divide_same_row_pair() {
        // Divide(2) on a same-row pair: [1,2] and [2,4] → both orderings each.
        let cage = Cage::new(4, pair(), Operation::Divide(2));
        let mut tuples = cage.tuples().to_vec();
        tuples.sort_unstable();
        assert_eq!(
            tuples,
            vec![vec![1u8, 2], vec![2, 1], vec![2, 4], vec![4, 2]]
        );
    }

    #[test]
    fn cage_new_multiply_same_row_pair() {
        // Multiply(6) on a same-row pair: multisets [1,6] and [2,3] → both orderings
        // each.
        let cage = Cage::new(6, pair(), Operation::Multiply(6));
        let mut tuples = cage.tuples().to_vec();
        tuples.sort_unstable();
        assert_eq!(
            tuples,
            vec![vec![1u8, 6], vec![2, 3], vec![3, 2], vec![6, 1]]
        );
    }

    #[test]
    fn cage_new_add_prunes_horizontal_pair() {
        // Add(6) on same-row pair: [2,4],[4,2],[3,3] — [3,3] filtered by row
        // collinearity.
        let cage = Cage::new(4, pair(), Operation::Add(6));
        let mut tuples = cage.tuples().to_vec();
        tuples.sort_unstable();
        assert_eq!(tuples, vec![vec![2u8, 4], vec![4, 2]]);
    }

    #[test]
    fn cage_new_add_prunes_l_shape() {
        // (0,0),(1,0),(1,1): col-0 pair (0,1), row-1 pair (1,2).
        // 10 raw permutations from {1,1,4},{1,2,3},{2,2,2} → 7 survive.
        let cage = Cage::new(4, l_shape(), Operation::Add(6));
        assert_eq!(cage.tuples().len(), 7);
        assert!(!cage.tuples().contains(&vec![1u8, 1, 4]));
        assert!(!cage.tuples().contains(&vec![2, 2, 2]));
    }

    // --- PartialOrd ---

    #[test]
    fn partial_cmp_orders_by_polyomino() {
        // singleton at (0,0) < pair starting at (0,0),(0,1) by lex cell order.
        let a = Cage::new(4, singleton(), Operation::Given(1));
        let b = Cage::new(4, pair(), Operation::Add(3));
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn cage_new_with_target_above_n_max_yields_no_tuples() {
        for (p, op) in [
            (singleton(), Operation::Given(300)),
            (pair(), Operation::Subtract(300)),
            (pair(), Operation::Divide(300)),
            (pair(), Operation::Add(300)),
        ] {
            assert!(Cage::new(4, p, op).tuples().is_empty());
        }
    }
}
