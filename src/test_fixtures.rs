pub mod fixtures {
    use crate::history::{DomainState, History};
    use crate::solver::SolvingStrategy;
    use crate::types::{Cage, Cell, LatinSquare, Operation, Puzzle};

    pub fn make_3x3_latin_square() -> LatinSquare {
        LatinSquare {
            n: 3,
            grid: vec![vec![2, 1, 3], vec![3, 2, 1], vec![1, 3, 2]],
        }
    }

    /// 3x3 puzzle with unique solution [2,1,3 / 3,2,1 / 1,3,2]:
    ///
    ///  +-------+---+
    ///  | 5+    | 4+|
    ///  +   +---+---+
    ///  |   | 2 | 2×|
    ///  +---+---+   |
    ///  | 2-|   |   |
    ///  +---+---+---+
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

    pub fn make_2x2_all_given_puzzle() -> Puzzle {
        Puzzle {
            latin_square: LatinSquare {
                n: 2,
                grid: vec![vec![1, 2], vec![2, 1]],
            },
            cages: vec![
                Cage {
                    cells: vec![(0, 0)],
                    op: Operation::Given(1),
                },
                Cage {
                    cells: vec![(0, 1)],
                    op: Operation::Given(2),
                },
                Cage {
                    cells: vec![(1, 0)],
                    op: Operation::Given(2),
                },
                Cage {
                    cells: vec![(1, 1)],
                    op: Operation::Given(1),
                },
            ],
        }
    }

    pub fn make_cage(cells: Vec<Cell>, op: Operation) -> Cage {
        Cage { cells, op }
    }

    pub struct AlwaysAcceptStrategy;

    impl SolvingStrategy for AlwaysAcceptStrategy {
        fn initial_state(&self, _puzzle: &Puzzle) -> DomainState {
            DomainState::default()
        }

        fn propagate(
            &self,
            _puzzle: &Puzzle,
            state: DomainState,
        ) -> (DomainState, History, bool) {
            (state, vec![], false)
        }

        fn branch(&self, _state: &DomainState) -> (DomainState, DomainState) {
            panic!("AlwaysAcceptStrategy never needs branching")
        }

        fn is_solved(&self, _state: &DomainState) -> bool {
            true
        }

        fn is_failed(&self, _state: &DomainState) -> bool {
            false
        }
    }

    pub struct AlwaysRejectStrategy;

    impl SolvingStrategy for AlwaysRejectStrategy {
        fn initial_state(&self, _puzzle: &Puzzle) -> DomainState {
            DomainState::default()
        }

        fn propagate(
            &self,
            _puzzle: &Puzzle,
            state: DomainState,
        ) -> (DomainState, History, bool) {
            (state, vec![], true)
        }

        fn branch(&self, _state: &DomainState) -> (DomainState, DomainState) {
            panic!("AlwaysRejectStrategy never needs branching")
        }

        fn is_solved(&self, _state: &DomainState) -> bool {
            false
        }

        fn is_failed(&self, _state: &DomainState) -> bool {
            true
        }
    }

}