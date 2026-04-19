use crate::history::{DomainState, History, SolveResult};
use crate::types::Puzzle;

pub trait SolvingStrategy {
    fn initial_state(&self, puzzle: &Puzzle) -> DomainState;
    fn propagate(&self, state: DomainState) -> (DomainState, History);
    fn branch(&self, _state: &DomainState) -> (DomainState, DomainState);
    fn is_solved(&self, state: &DomainState) -> bool;
    fn is_failed(&self, state: &DomainState) -> bool;
}

pub fn solve(puzzle: &Puzzle, s: &dyn SolvingStrategy) -> (SolveResult, History) {
    let state = s.initial_state(puzzle);
    solve_inner(state, s)
}

pub fn solve_inner(state: DomainState, s: &dyn SolvingStrategy) -> (SolveResult, History) {
    let (state, mut h0) = s.propagate(state);
    if s.is_failed(&state) {
        return (SolveResult::NoSolution, h0);
    }
    if s.is_solved(&state) {
        return (SolveResult::Unique(state), h0);
    }
    let (left, right) = s.branch(&state);
    let (r1, h1) = solve_inner(left, s);
    match r1 {
        SolveResult::NoSolution => {
            let (r2, h2) = solve_inner(right, s);
            h0.extend(h1);
            h0.extend(h2);
            (r2, h0)
        }
        SolveResult::Unique(sol1) => {
            let (r2, h2) = solve_inner(right, s);
            h0.extend(h1);
            h0.extend(h2);
            match r2 {
                SolveResult::NoSolution => (SolveResult::Unique(sol1), h0),
                SolveResult::Unique(sol2) => (SolveResult::NonUnique(sol1, sol2), h0),
                SolveResult::NonUnique(..) => (r2, h0),
            }
        }
        SolveResult::NonUnique(..) => {
            // Early exit: no need to explore right branch
            h0.extend(h1);
            (r1, h0)
        }
    }
}

/// A strategy that works only on puzzles where every cage is `Operation::Given`.
/// Each cell's domain is initialized to the singleton set containing its given value.
pub struct TrivialStrategy;

impl SolvingStrategy for TrivialStrategy {
    fn initial_state(&self, puzzle: &Puzzle) -> DomainState {
        use crate::types::Operation;
        use std::collections::{BTreeSet, HashMap};

        let mut cell_domains: HashMap<crate::types::Cell, BTreeSet<crate::types::Value>> =
            HashMap::new();
        for cage in &puzzle.cages {
            if let Operation::Given(v) = cage.op {
                for &cell in &cage.cells {
                    cell_domains.insert(cell, BTreeSet::from([v]));
                }
            }
        }
        DomainState { cell_domains }
    }

    fn propagate(&self, state: DomainState) -> (DomainState, History) {
        (state, vec![])
    }

    fn branch(&self, _state: &DomainState) -> (DomainState, DomainState) {
        panic!("TrivialStrategy never needs branching")
    }

    fn is_solved(&self, state: &DomainState) -> bool {
        !state.cell_domains.is_empty() && state.cell_domains.values().all(|d| d.len() == 1)
    }

    fn is_failed(&self, state: &DomainState) -> bool {
        state.cell_domains.values().any(|d| d.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::SolveResult;
    use crate::types::{Cage, LatinSquare, Operation, Puzzle};

    fn make_2x2_all_given_puzzle() -> Puzzle {
        // Latin square: [[1,2],[2,1]]
        let latin_square = LatinSquare {
            n: 2,
            grid: vec![vec![1, 2], vec![2, 1]],
        };
        let cages = vec![
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
        ];
        Puzzle {
            latin_square,
            cages,
        }
    }

    #[test]
    fn trivial_strategy_solves_all_given_puzzle() {
        let puzzle = make_2x2_all_given_puzzle();
        let (result, _history) = solve(&puzzle, &TrivialStrategy);
        assert!(matches!(result, SolveResult::Unique(_)));
    }

    #[test]
    fn solve_returns_no_solution_for_failed_state() {
        use crate::history::DomainState;
        use std::collections::{BTreeSet, HashMap};

        struct FailedStrategy;
        impl SolvingStrategy for FailedStrategy {
            fn initial_state(&self, _puzzle: &Puzzle) -> DomainState {
                // A state with an empty domain for a cell — already failed
                let mut cell_domains = HashMap::new();
                cell_domains.insert((0, 0), BTreeSet::new());
                DomainState { cell_domains }
            }
            fn propagate(&self, state: DomainState) -> (DomainState, History) {
                (state, vec![])
            }
            fn branch(&self, _state: &DomainState) -> (DomainState, DomainState) {
                panic!("should never branch a failed state")
            }
            fn is_solved(&self, _state: &DomainState) -> bool {
                false
            }
            fn is_failed(&self, state: &DomainState) -> bool {
                state.cell_domains.values().any(|d| d.is_empty())
            }
        }

        let puzzle = make_2x2_all_given_puzzle();
        let (result, _history) = solve(&puzzle, &FailedStrategy);
        assert!(matches!(result, SolveResult::NoSolution));
    }

    #[test]
    fn solve_inner_stops_early_on_non_unique() {
        use crate::history::DomainState;
        use std::collections::{BTreeSet, HashMap};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        // CountingStrategy: always returns two distinct solutions and counts branch calls.
        // Panics if branch is called more than twice (i.e., the third branch is explored).
        struct CountingStrategy {
            branch_count: Arc<AtomicUsize>,
        }

        impl SolvingStrategy for CountingStrategy {
            fn initial_state(&self, _puzzle: &Puzzle) -> DomainState {
                // A non-solved, non-failed state with two possible values for (0,0)
                let mut cell_domains: HashMap<_, BTreeSet<_>> = HashMap::new();
                cell_domains.insert((0, 0), [1u8, 2u8].iter().copied().collect());
                DomainState { cell_domains }
            }

            fn propagate(&self, state: DomainState) -> (DomainState, History) {
                (state, vec![])
            }

            fn branch(&self, _state: &DomainState) -> (DomainState, DomainState) {
                let count = self.branch_count.fetch_add(1, Ordering::SeqCst);
                assert!(
                    count < 2,
                    "branch called more than twice — early exit failed"
                );

                // Left branch: domain = {1} (a solved state)
                let mut left_domains: HashMap<_, BTreeSet<_>> = HashMap::new();
                left_domains.insert((0, 0), [1u8].iter().copied().collect());

                // Right branch: domain = {2} (another solved state)
                let mut right_domains: HashMap<_, BTreeSet<_>> = HashMap::new();
                right_domains.insert((0, 0), [2u8].iter().copied().collect());

                (
                    DomainState {
                        cell_domains: left_domains,
                    },
                    DomainState {
                        cell_domains: right_domains,
                    },
                )
            }

            fn is_solved(&self, state: &DomainState) -> bool {
                state.cell_domains.values().all(|d| d.len() == 1)
            }

            fn is_failed(&self, _state: &DomainState) -> bool {
                false
            }
        }

        let counter = Arc::new(AtomicUsize::new(0));
        let strategy = CountingStrategy {
            branch_count: Arc::clone(&counter),
        };
        let puzzle = make_2x2_all_given_puzzle();
        let (result, _history) = solve(&puzzle, &strategy);
        assert!(matches!(result, SolveResult::NonUnique(_, _)));
        // branch was called exactly once (only the root level; sub-states are already solved)
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
