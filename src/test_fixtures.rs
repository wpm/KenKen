pub mod fixtures {
    use crate::puzzle::{Cage, Grid, Operation, Puzzle};
    use std::collections::BTreeSet;

    pub fn make_3x3_latin_square() -> Grid {
        Grid {
            grid: vec![vec![2, 1, 3], vec![3, 2, 1], vec![1, 3, 2]],
        }
    }

    /// 3x3 puzzle with unique solution \[2,1,3 / 3,2,1 / 1,3,2\].
    ///
    /// ```text
    ///  +-------+---+
    ///  | 5+    | 4+|
    ///  +   +---+---+
    ///  |   | 2 | 2×|
    ///  +---+---+   |
    ///  | 2-|   |   |
    ///  +---+---+---+
    /// ```
    pub fn make_3x3_unique_puzzle() -> Puzzle {
        Puzzle {
            latin_square: make_3x3_latin_square(),
            cages: BTreeSet::from([
                Cage {
                    cells: BTreeSet::from([(0, 0), (1, 0)]),
                    op: Operation::Add(5),
                },
                Cage {
                    cells: BTreeSet::from([(0, 1), (0, 2)]),
                    op: Operation::Add(4),
                },
                Cage {
                    cells: BTreeSet::from([(1, 1)]),
                    op: Operation::Given(2),
                },
                Cage {
                    cells: BTreeSet::from([(1, 2), (2, 2)]),
                    op: Operation::Mul(2),
                },
                Cage {
                    cells: BTreeSet::from([(2, 0), (2, 1)]),
                    op: Operation::Sub(2),
                },
            ]),
        }
    }
}
