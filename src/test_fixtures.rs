pub mod fixtures {
    use crate::types::{Cage, LatinSquare, Operation, Puzzle};

    pub fn make_3x3_latin_square() -> LatinSquare {
        LatinSquare {
            n: 3,
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
            cages: vec![
                Cage {
                    cells: vec![(0, 0), (1, 0)],
                    op: Operation::Add(5),
                },
                Cage {
                    cells: vec![(0, 1), (0, 2)],
                    op: Operation::Add(4),
                },
                Cage {
                    cells: vec![(1, 1)],
                    op: Operation::Given(2),
                },
                Cage {
                    cells: vec![(1, 2), (2, 2)],
                    op: Operation::Mul(2),
                },
                Cage {
                    cells: vec![(2, 0), (2, 1)],
                    op: Operation::Sub(2),
                },
            ],
        }
    }
}
