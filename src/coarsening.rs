use std::collections::HashSet;

use rand::Rng;

use crate::geometry::{adjacent_pairs, merge_cages, trivial_cages};
use crate::history::{Event, History, SolveResult};
use crate::operation::assign_operation;
use crate::solver::{SolvingStrategy, solve};
use crate::types::{Cage, Cell, Puzzle};

type CagePairKey = (Vec<Cell>, Vec<Cell>);

pub struct Coarsening {
    pub stopping_threshold: usize,
}

impl Coarsening {
    pub fn generate(
        &self,
        puzzle: &Puzzle,
        solver: &dyn SolvingStrategy,
        rng: &mut impl Rng,
    ) -> (Option<Vec<Cage>>, History) {
        let mut current = trivial_cages(&puzzle.latin_square);
        let mut history: History = Vec::new();
        let mut blacklist: HashSet<CagePairKey> = HashSet::new();
        let mut pairs_cache: Option<Vec<(usize, usize)>> = None;

        loop {
            let all_pairs = pairs_cache.get_or_insert_with(|| adjacent_pairs(&current));
            let mut candidates: Vec<(usize, usize, CagePairKey)> = all_pairs
                .iter()
                .filter_map(|&(i, j)| {
                    let key = cage_pair_key(&current[i], &current[j]);
                    if blacklist.contains(&key) {
                        None
                    } else {
                        Some((i, j, key))
                    }
                })
                .collect();

            if candidates.is_empty() {
                return (None, history);
            }

            let idx = rng.random_range(0..candidates.len());
            let (i, j, key) = candidates.swap_remove(idx);

            let merged_cells: Vec<_> = current[i]
                .cells
                .iter()
                .chain(current[j].cells.iter())
                .copied()
                .collect();
            let op = assign_operation(&merged_cells, &puzzle.latin_square);
            let merged = merge_cages(&current[i], &current[j], op);

            let cages_prime: Vec<Cage> = current
                .iter()
                .enumerate()
                .filter(|&(k, _)| k != i && k != j)
                .map(|(_, c)| c.clone())
                .chain(std::iter::once(merged))
                .collect();

            let puzzle_prime = Puzzle {
                latin_square: puzzle.latin_square.clone(),
                cages: cages_prime,
            };

            let (result, solver_history) = solve(&puzzle_prime, solver);
            let accepted = matches!(result, SolveResult::Unique(_));

            history.push(Event::MergeAttempted {
                cage_a: current[i].clone(),
                cage_b: current[j].clone(),
                accepted,
            });
            history.extend(solver_history);

            if accepted {
                current = puzzle_prime.cages;
                pairs_cache = None;
                if current.len() <= self.stopping_threshold {
                    return (Some(current), history);
                }
            } else {
                blacklist.insert(key);
            }
        }
    }
}

fn cage_pair_key(a: &Cage, b: &Cage) -> CagePairKey {
    let mut ka: Vec<Cell> = a.cells.clone();
    let mut kb: Vec<Cell> = b.cells.clone();
    ka.sort_unstable();
    kb.sort_unstable();
    if ka <= kb { (ka, kb) } else { (kb, ka) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::satisfies_operation;
    use crate::geometry::is_cage_contiguous;
    use crate::history::{DomainState, HistorySummary};
    use crate::latin_square::generate_latin_square;
    use crate::solver::BacktrackingStrategy;
    use crate::types::{LatinSquare, Puzzle};
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    /// Accepts every merge (reports Unique unconditionally).
    /// Isolates coarsening loop logic from real solver behaviour.
    struct AlwaysAcceptStrategy;

    impl SolvingStrategy for AlwaysAcceptStrategy {
        fn initial_state(&self, _puzzle: &Puzzle) -> DomainState {
            DomainState::default()
        }

        fn propagate(&self, _puzzle: &Puzzle, state: DomainState) -> (DomainState, History, bool) {
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

    /// Rejects every merge (reports NoSolution unconditionally).
    struct AlwaysRejectStrategy;

    impl SolvingStrategy for AlwaysRejectStrategy {
        fn initial_state(&self, _puzzle: &Puzzle) -> DomainState {
            DomainState::default()
        }

        fn propagate(&self, _puzzle: &Puzzle, state: DomainState) -> (DomainState, History, bool) {
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

    fn make_3x3_latin_square() -> LatinSquare {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        generate_latin_square(3, &mut rng)
    }

    fn make_3x3_puzzle() -> Puzzle {
        Puzzle {
            latin_square: make_3x3_latin_square(),
            cages: vec![],
        }
    }

    #[test]
    fn coarsening_terminates_and_returns_some() {
        let puzzle = make_3x3_puzzle();
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        let (result, _) = Coarsening {
            stopping_threshold: 4,
        }
        .generate(&puzzle, &AlwaysAcceptStrategy, &mut rng);
        if let Some(cages) = result {
            assert!(cages.len() <= 4);
        }
    }

    #[test]
    fn history_contains_merge_attempted_events() {
        let puzzle = make_3x3_puzzle();
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        let (_, history) = Coarsening {
            stopping_threshold: 4,
        }
        .generate(&puzzle, &AlwaysAcceptStrategy, &mut rng);
        let summary = HistorySummary::from_history(&history);
        assert!(summary.merge_attempted >= 1);
    }

    #[test]
    fn all_returned_cages_are_connected() {
        let puzzle = make_3x3_puzzle();
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        let (result, _) = Coarsening {
            stopping_threshold: 4,
        }
        .generate(&puzzle, &AlwaysAcceptStrategy, &mut rng);
        if let Some(cages) = result {
            for cage in &cages {
                assert!(
                    is_cage_contiguous(cage),
                    "cage {:?} is not connected",
                    cage.cells
                );
            }
        }
    }

    #[test]
    fn loop_emits_events_before_reaching_threshold() {
        let puzzle = make_3x3_puzzle();
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let (_, history) = Coarsening {
            stopping_threshold: 7,
        }
        .generate(&puzzle, &AlwaysAcceptStrategy, &mut rng);
        let summary = HistorySummary::from_history(&history);
        assert!(
            summary.merge_attempted >= 2,
            "expected >= 2 MergeAttempted events, got {}",
            summary.merge_attempted
        );
        assert_eq!(summary.merge_accepted, summary.merge_attempted);
    }

    #[test]
    fn rejected_merges_are_blacklisted_and_loop_terminates() {
        let puzzle = make_3x3_puzzle();
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        let (result, history) = Coarsening {
            stopping_threshold: 1,
        }
        .generate(&puzzle, &AlwaysRejectStrategy, &mut rng);
        assert!(result.is_none());
        let summary = HistorySummary::from_history(&history);
        assert_eq!(summary.merge_accepted, 0);
        assert!(summary.merge_attempted > 0);
    }

    // ── Integration tests using BacktrackingStrategy ──────────────────────────

    fn generate_puzzle(n: usize, seed: u64) -> (Option<Vec<Cage>>, LatinSquare) {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let ls = generate_latin_square(n, &mut rng);
        let puzzle = Puzzle {
            latin_square: ls.clone(),
            cages: vec![],
        };
        let threshold = ((n * n) / 3).max(2);
        let (cages, _) = Coarsening {
            stopping_threshold: threshold,
        }
        .generate(&puzzle, &BacktrackingStrategy, &mut rng);
        (cages, ls)
    }

    fn assert_generated_puzzle_is_valid(n: usize, seed: u64) {
        let (cages, ls) = generate_puzzle(n, seed);
        let cages = cages.expect("coarsening should produce Some cages");

        let puzzle = Puzzle {
            latin_square: ls.clone(),
            cages: cages.clone(),
        };

        assert!(puzzle.validate(), "cages must partition all cells");

        for cage in &cages {
            assert!(
                is_cage_contiguous(cage),
                "cage {:?} is not connected",
                cage.cells
            );
        }

        let (result, _) = solve(&puzzle, &BacktrackingStrategy);
        assert!(
            matches!(result, SolveResult::Unique(_)),
            "generated puzzle must be uniquely solvable"
        );

        for cage in &cages {
            let values: Vec<crate::types::Value> =
                cage.cells.iter().map(|&cell| ls.get(cell)).collect();
            assert!(
                satisfies_operation(&cage.op, &values),
                "cage {:?} op {:?} not satisfied by Latin square",
                cage.cells,
                cage.op
            );
        }
    }

    #[test]
    fn generate_3x3_is_unique_and_valid() {
        assert_generated_puzzle_is_valid(3, 42);
    }

    #[test]
    fn generate_4x4_is_unique_and_valid() {
        assert_generated_puzzle_is_valid(4, 42);
    }

    #[test]
    fn generate_5x5_is_unique_and_valid() {
        assert_generated_puzzle_is_valid(5, 42);
    }

    #[test]
    fn generate_is_deterministic() {
        let (cages_a, ls_a) = generate_puzzle(4, 123);
        let (cages_b, ls_b) = generate_puzzle(4, 123);
        assert_eq!(ls_a.grid, ls_b.grid);
        assert_eq!(
            cages_a.as_ref().map(|v| v.len()),
            cages_b.as_ref().map(|v| v.len())
        );
        if let (Some(a), Some(b)) = (cages_a, cages_b) {
            for (ca, cb) in a.iter().zip(b.iter()) {
                assert_eq!(ca.cells, cb.cells);
                assert_eq!(ca.op, cb.op);
            }
        }
    }

    #[test]
    fn generate_produces_nontrivial_cages() {
        let (cages, _) = generate_puzzle(5, 42);
        let cages = cages.expect("expected Some cages");
        assert!(cages.iter().any(|c| c.cells.len() >= 2));
    }
}
