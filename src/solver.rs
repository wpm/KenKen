use std::collections::{BTreeMap, BTreeSet};

use crate::domain::satisfies_operation;
use crate::geometry::conflict_graph;
use crate::history::{DomainState, History, SolveResult};
use crate::types::{Cell, Operation, Puzzle, Value};

pub trait SolvingStrategy {
    fn initial_state(&self, puzzle: &Puzzle) -> DomainState;
    /// Propagate constraints. Returns `(state, history, failed)` where `failed`
    /// is true if any cell domain was emptied during propagation.
    fn propagate(&self, puzzle: &Puzzle, state: DomainState) -> (DomainState, History, bool);
    fn branch(&self, _state: &DomainState) -> (DomainState, DomainState);
    fn is_solved(&self, state: &DomainState) -> bool;
    fn is_failed(&self, state: &DomainState) -> bool;
}

pub fn solve(puzzle: &Puzzle, s: &dyn SolvingStrategy) -> (SolveResult, History) {
    let state = s.initial_state(puzzle);
    solve_inner(state, puzzle, s)
}

pub fn solve_inner(
    state: DomainState,
    puzzle: &Puzzle,
    s: &dyn SolvingStrategy,
) -> (SolveResult, History) {
    let (state, mut h0, failed) = s.propagate(puzzle, state);
    if failed {
        return (SolveResult::NoSolution, h0);
    }
    if s.is_solved(&state) {
        return (SolveResult::Unique(state), h0);
    }
    let (left, right) = s.branch(&state);
    let (r1, h1) = solve_inner(left, puzzle, s);
    match r1 {
        SolveResult::NoSolution => {
            let (r2, h2) = solve_inner(right, puzzle, s);
            h0.extend(h1);
            h0.extend(h2);
            (r2, h0)
        }
        SolveResult::Unique(sol1) => {
            let (r2, h2) = solve_inner(right, puzzle, s);
            h0.extend(h1);
            h0.extend(h2);
            match r2 {
                SolveResult::NoSolution => (SolveResult::Unique(sol1), h0),
                SolveResult::Unique(sol2) => (SolveResult::NonUnique(sol1, sol2), h0),
                SolveResult::NonUnique(..) => (r2, h0),
            }
        }
        // NonUnique is absorbing: a third solution cannot change the result.
        SolveResult::NonUnique(..) => {
            h0.extend(h1);
            (r1, h0)
        }
    }
}

/// Primal backtracking strategy: one cell variable per cell, domain {1..=n}.
///
/// Propagation enforces Latin-square uniqueness and cage arithmetic constraints
/// to fixpoint before branching. Branching uses MRV (minimum remaining values).
pub struct BacktrackingStrategy;

impl BacktrackingStrategy {
    fn propagate_puzzle(puzzle: &Puzzle, mut state: DomainState) -> (DomainState, bool) {
        let n = puzzle.latin_square.n;
        // Conflict graphs are cage-structural and don't change during propagation.
        let cage_conflicts: Vec<Vec<(usize, usize)>> =
            puzzle.cages.iter().map(conflict_graph).collect();
        let mut buf: Vec<Value> = Vec::with_capacity(n);
        loop {
            let mut changed = false;

            for line in 0..n {
                let cell_fns: [&dyn Fn(usize) -> (usize, usize); 2] =
                    [&|i| (line, i), &|i| (i, line)];
                for cell_fn in cell_fns {
                    buf.clear();
                    buf.extend((0..n).filter_map(|i| {
                        let d = state.cell_domains.get(&cell_fn(i))?;
                        if d.len() == 1 { d.iter().next().copied() } else { None }
                    }));
                    for &v in buf.iter() {
                        for i in 0..n {
                            let d = state.cell_domains.entry(cell_fn(i)).or_default();
                            if d.len() > 1 && d.remove(&v) {
                                changed = true;
                            }
                        }
                    }
                }
            }

            for (cage, conflicts) in puzzle.cages.iter().zip(&cage_conflicts) {
                let k = cage.cells.len();
                let domains: Vec<BTreeSet<Value>> = cage
                    .cells
                    .iter()
                    .map(|cell| state.cell_domains.get(cell).cloned().unwrap_or_default())
                    .collect();
                let mut reachable: Vec<BTreeSet<Value>> = vec![BTreeSet::new(); k];
                let partial0 = if matches!(&cage.op, Operation::Mul(_)) {
                    1
                } else {
                    0
                };
                enumerate_valid(
                    &cage.op,
                    &domains,
                    conflicts,
                    partial0,
                    &mut Vec::with_capacity(k),
                    &mut reachable,
                );

                for (pos, cell) in cage.cells.iter().enumerate() {
                    let d = state.cell_domains.entry(*cell).or_default();
                    let before_len = d.len();
                    d.retain(|v| reachable[pos].contains(v));
                    if d.len() != before_len {
                        changed = true;
                    }
                }
            }

            if !changed {
                break;
            }
        }

        let failed = state.cell_domains.values().any(|d| d.is_empty());
        (state, failed)
    }
}

/// `partial` accumulates operation-specific state for early pruning:
/// Add → running sum (prune when > target); Mul → running product (prune when > target).
/// Caller must pass 0 for Add and 1 for Mul (multiplicative identity).
fn enumerate_valid(
    op: &Operation,
    domains: &[BTreeSet<Value>],
    conflicts: &[(usize, usize)],
    partial: u32,
    current: &mut Vec<Value>,
    reachable: &mut Vec<BTreeSet<Value>>,
) {
    let pos = current.len();
    if pos == domains.len() {
        if satisfies_operation(op, current) {
            for (i, &v) in current.iter().enumerate() {
                reachable[i].insert(v);
            }
        }
        return;
    }
    'outer: for &v in &domains[pos] {
        // Skip if an earlier conflicting position already holds this value.
        for &(i, j) in conflicts {
            if j == pos && current[i] == v {
                continue 'outer;
            }
        }
        let next_partial = match op {
            Operation::Add(t) => {
                let s = partial + v as u32;
                if s > *t {
                    continue;
                }
                s
            }
            Operation::Mul(t) => {
                let p = partial * v as u32;
                if p > *t {
                    continue;
                }
                p
            }
            _ => 0,
        };
        current.push(v);
        enumerate_valid(op, domains, conflicts, next_partial, current, reachable);
        current.pop();
    }
}

impl SolvingStrategy for BacktrackingStrategy {
    fn initial_state(&self, puzzle: &Puzzle) -> DomainState {
        let n = puzzle.latin_square.n;
        let full: BTreeSet<Value> = (1..=(n as Value)).collect();
        let cell_domains: BTreeMap<Cell, BTreeSet<Value>> = (0..n)
            .flat_map(|r| {
                let full = full.clone();
                (0..n).map(move |c| ((r, c), full.clone()))
            })
            .collect();
        DomainState { cell_domains }
    }

    fn propagate(&self, puzzle: &Puzzle, state: DomainState) -> (DomainState, History, bool) {
        let (state, failed) = BacktrackingStrategy::propagate_puzzle(puzzle, state);
        (state, vec![], failed)
    }

    fn branch(&self, state: &DomainState) -> (DomainState, DomainState) {
        let (&cell, domain) = state
            .cell_domains
            .iter()
            .filter(|(_, d)| d.len() > 1)
            .min_by_key(|(_, d)| d.len())
            .expect("branch called on a solved or failed state");

        let &v = domain.iter().next().unwrap();

        let mut left = state.clone();
        left.cell_domains
            .get_mut(&cell)
            .unwrap()
            .retain(|&x| x == v);

        let mut right = state.clone();
        right.cell_domains.get_mut(&cell).unwrap().remove(&v);

        (left, right)
    }

    fn is_solved(&self, state: &DomainState) -> bool {
        state.is_solved()
    }

    fn is_failed(&self, state: &DomainState) -> bool {
        state.is_failed()
    }
}

/// Only works on puzzles where every cage is `Operation::Given`.
/// Each cell's domain is initialized to the singleton `{given value}`.
pub struct TrivialStrategy;

impl SolvingStrategy for TrivialStrategy {
    fn initial_state(&self, puzzle: &Puzzle) -> DomainState {
        let mut cell_domains: BTreeMap<Cell, BTreeSet<Value>> = BTreeMap::new();
        for cage in &puzzle.cages {
            if let Operation::Given(v) = cage.op {
                for &cell in &cage.cells {
                    cell_domains.insert(cell, BTreeSet::from([v]));
                }
            }
        }
        DomainState { cell_domains }
    }

    fn propagate(&self, _puzzle: &Puzzle, state: DomainState) -> (DomainState, History, bool) {
        let failed = self.is_failed(&state);
        (state, vec![], failed)
    }

    fn branch(&self, _state: &DomainState) -> (DomainState, DomainState) {
        panic!("TrivialStrategy never needs branching")
    }

    fn is_solved(&self, state: &DomainState) -> bool {
        state.is_solved()
    }

    fn is_failed(&self, state: &DomainState) -> bool {
        state.is_failed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::SolveResult;
    use crate::test_fixtures::fixtures::{make_2x2_all_given_puzzle, make_3x3_unique_puzzle};
    use crate::types::{Cage, LatinSquare, Operation, Puzzle};

    // 3x3 puzzle with 12 solutions: one row-wide Add(6) cage per row.
    fn make_3x3_non_unique_puzzle() -> Puzzle {
        let latin_square = LatinSquare {
            n: 3,
            grid: vec![vec![1, 2, 3], vec![2, 3, 1], vec![3, 1, 2]],
        };
        Puzzle {
            latin_square: latin_square.clone(),
            cages: (0..3)
                .map(|r| Cage {
                    cells: (0..3).map(|c| (r, c)).collect(),
                    op: Operation::Add(6),
                })
                .collect(),
        }
    }

    #[test]
    fn backtracking_solves_unique_3x3() {
        let puzzle = make_3x3_unique_puzzle();
        let (result, _) = solve(&puzzle, &BacktrackingStrategy);
        match result {
            SolveResult::Unique(state) => {
                let expected = [vec![2u8, 1, 3], vec![3, 2, 1], vec![1, 3, 2]];
                for (r, row) in expected.iter().enumerate() {
                    for (c, &v) in row.iter().enumerate() {
                        let d = &state.cell_domains[&(r, c)];
                        assert_eq!(d.len(), 1);
                        assert_eq!(*d.iter().next().unwrap(), v);
                    }
                }
            }
            other => panic!("expected Unique, got {:?}", other),
        }
    }

    #[test]
    fn backtracking_detects_non_unique_puzzle() {
        let puzzle = make_3x3_non_unique_puzzle();
        let (result, _) = solve(&puzzle, &BacktrackingStrategy);
        assert!(matches!(result, SolveResult::NonUnique(_, _)));
    }

    #[test]
    fn backtracking_solves_4x4_after_one_merge() {
        use crate::geometry::{adjacent_pairs, merge_cages, replace_with_merged, trivial_cages};
        use crate::latin_square::generate_latin_square;
        use crate::operation::assign_operation;
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let ls = generate_latin_square(4, &mut rng);
        let cages = trivial_cages(&ls);
        let pairs = adjacent_pairs(&cages);
        let (i, j) = pairs[0];

        let merged_cells: Vec<_> = cages[i]
            .cells
            .iter()
            .chain(cages[j].cells.iter())
            .copied()
            .collect();
        let op = assign_operation(&merged_cells, &ls);
        let merged = merge_cages(&cages[i], &cages[j], op);
        let new_cages = replace_with_merged(&cages, i, j, merged);

        let puzzle = Puzzle {
            latin_square: ls,
            cages: new_cages,
        };
        let (result, _) = solve(&puzzle, &BacktrackingStrategy);
        assert!(matches!(result, SolveResult::Unique(_)));
    }

    #[test]
    fn backtracking_returns_no_solution_for_impossible_puzzle() {
        let latin_square = LatinSquare {
            n: 2,
            grid: vec![vec![1, 2], vec![2, 1]],
        };
        let puzzle = Puzzle {
            latin_square,
            cages: vec![Cage {
                cells: vec![(0, 0), (0, 1), (1, 0), (1, 1)],
                op: Operation::Add(99),
            }],
        };
        let (result, _) = solve(&puzzle, &BacktrackingStrategy);
        assert!(matches!(result, SolveResult::NoSolution));
    }

    #[test]
    fn trivial_strategy_solves_all_given_puzzle() {
        let puzzle = make_2x2_all_given_puzzle();
        let (result, _) = solve(&puzzle, &TrivialStrategy);
        assert!(matches!(result, SolveResult::Unique(_)));
    }

    #[test]
    fn solve_returns_no_solution_for_failed_state() {
        struct FailedStrategy;
        impl SolvingStrategy for FailedStrategy {
            fn initial_state(&self, _puzzle: &Puzzle) -> DomainState {
                let mut cell_domains = BTreeMap::new();
                cell_domains.insert((0, 0), BTreeSet::new());
                DomainState { cell_domains }
            }
            fn propagate(
                &self,
                _puzzle: &Puzzle,
                state: DomainState,
            ) -> (DomainState, History, bool) {
                let failed = self.is_failed(&state);
                (state, vec![], failed)
            }
            fn branch(&self, _state: &DomainState) -> (DomainState, DomainState) {
                panic!("should never branch a failed state")
            }
            fn is_solved(&self, _state: &DomainState) -> bool {
                false
            }
            fn is_failed(&self, state: &DomainState) -> bool {
                state.is_failed()
            }
        }

        let puzzle = make_2x2_all_given_puzzle();
        let (result, _) = solve(&puzzle, &FailedStrategy);
        assert!(matches!(result, SolveResult::NoSolution));
    }

    #[test]
    fn solve_inner_stops_early_on_non_unique() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingStrategy {
            branch_count: Arc<AtomicUsize>,
        }

        impl SolvingStrategy for CountingStrategy {
            fn initial_state(&self, _puzzle: &Puzzle) -> DomainState {
                let mut cell_domains: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
                cell_domains.insert((0, 0), [1u8, 2u8].iter().copied().collect());
                DomainState { cell_domains }
            }

            fn propagate(
                &self,
                _puzzle: &Puzzle,
                state: DomainState,
            ) -> (DomainState, History, bool) {
                (state, vec![], false)
            }

            fn branch(&self, _state: &DomainState) -> (DomainState, DomainState) {
                let count = self.branch_count.fetch_add(1, Ordering::SeqCst);
                assert!(
                    count < 2,
                    "branch called more than twice — early exit failed"
                );

                let mut left_domains: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
                left_domains.insert((0, 0), [1u8].iter().copied().collect());
                let mut right_domains: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
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
                state.is_solved()
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
        let (result, _) = solve(&puzzle, &strategy);
        assert!(matches!(result, SolveResult::NonUnique(_, _)));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
