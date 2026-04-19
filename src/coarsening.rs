use rand::Rng;

use crate::geometry::{adjacent_pairs, merge_cages, trivial_cages};
use crate::history::{Event, History, SolveResult};
use crate::operation::assign_operation;
use crate::solver::{SolvingStrategy, solve};
use crate::types::{Cage, Puzzle};

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

        loop {
            let candidates = adjacent_pairs(&current);
            if candidates.is_empty() {
                return (None, history);
            }

            let idx = rng.random_range(0..candidates.len());
            let (i, j) = candidates[idx];

            let temp = merge_cages(&current[i], &current[j], crate::types::Operation::Add(0));
            let op = assign_operation(&temp, &puzzle.latin_square);
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
                if current.len() <= self.stopping_threshold {
                    return (Some(current), history);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{DomainState, HistorySummary};
    use crate::latin_square::generate_latin_square;
    use crate::types::Puzzle;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    /// A strategy that always reports the puzzle as uniquely solved.
    /// This lets us test the coarsening loop logic independently of any real solver.
    struct AlwaysUniqueStrategy;

    impl SolvingStrategy for AlwaysUniqueStrategy {
        fn initial_state(&self, _puzzle: &Puzzle) -> DomainState {
            DomainState::default()
        }

        fn propagate(&self, state: DomainState) -> (DomainState, History) {
            (state, vec![])
        }

        fn branch(&self, _state: &DomainState) -> (DomainState, DomainState) {
            panic!("AlwaysUniqueStrategy never needs branching")
        }

        fn is_solved(&self, _state: &DomainState) -> bool {
            true
        }

        fn is_failed(&self, _state: &DomainState) -> bool {
            false
        }
    }

    fn make_3x3_puzzle() -> Puzzle {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let ls = generate_latin_square(3, &mut rng);
        Puzzle {
            latin_square: ls,
            cages: vec![],
        }
    }

    #[test]
    fn coarsening_terminates_and_returns_some() {
        let puzzle = make_3x3_puzzle();
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        let (result, _history) = Coarsening {
            stopping_threshold: 4,
        }
        .generate(&puzzle, &AlwaysUniqueStrategy, &mut rng);
        // Either we reduced to <= 4 cages, or we ran out of adjacent pairs.
        // Both outcomes are valid; we just assert it terminates and returns a result.
        if let Some(cages) = result {
            assert!(cages.len() <= 4);
        }
    }

    #[test]
    fn history_contains_merge_attempted_events() {
        let puzzle = make_3x3_puzzle();
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        let (_result, history) = Coarsening {
            stopping_threshold: 4,
        }
        .generate(&puzzle, &AlwaysUniqueStrategy, &mut rng);
        let summary = HistorySummary::from_history(&history);
        assert!(
            summary.merge_attempted >= 1,
            "expected at least one MergeAttempted event"
        );
    }

    #[test]
    fn all_returned_cages_are_connected() {
        let puzzle = make_3x3_puzzle();
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        let (result, _history) = Coarsening {
            stopping_threshold: 4,
        }
        .generate(&puzzle, &AlwaysUniqueStrategy, &mut rng);
        if let Some(cages) = result {
            for cage in &cages {
                assert!(
                    is_cage_connected(cage),
                    "cage {:?} is not connected",
                    cage.cells
                );
            }
        }
    }

    /// BFS connectivity check for `types::Cage`.
    fn is_cage_connected(cage: &Cage) -> bool {
        use std::collections::{HashSet, VecDeque};
        if cage.cells.len() <= 1 {
            return true;
        }
        let cell_set: HashSet<_> = cage.cells.iter().copied().collect();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(cage.cells[0]);
        visited.insert(cage.cells[0]);
        while let Some((r, c)) = queue.pop_front() {
            for nb in [
                (r.wrapping_sub(1), c),
                (r + 1, c),
                (r, c.wrapping_sub(1)),
                (r, c + 1),
            ] {
                if cell_set.contains(&nb) && visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        visited.len() == cage.cells.len()
    }

    #[test]
    fn loop_emits_events_before_reaching_threshold() {
        // With AlwaysUniqueStrategy every merge is accepted.
        // Starting from 9 trivial cages for a 3x3, we should get multiple
        // MergeAttempted events as the count decreases toward the threshold.
        let puzzle = make_3x3_puzzle();
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let (_result, history) = Coarsening {
            stopping_threshold: 7,
        }
        .generate(&puzzle, &AlwaysUniqueStrategy, &mut rng);
        let summary = HistorySummary::from_history(&history);
        // At least 2 merges should have been attempted (9 → 8 → 7, then stop)
        assert!(
            summary.merge_attempted >= 2,
            "expected >= 2 MergeAttempted events, got {}",
            summary.merge_attempted
        );
        assert_eq!(
            summary.merge_accepted, summary.merge_attempted,
            "AlwaysUniqueStrategy should accept every merge"
        );
    }
}
