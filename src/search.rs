use std::collections::BTreeSet;
use std::ops::ControlFlow;

use crate::puzzle::Puzzle;

impl Puzzle {
    /// Enumerates solutions in depth-first order, calling `f` on each fully solved
    /// puzzle. Returns `Break(())` if `f` requested termination, else `Continue(())`
    /// once the search tree is exhausted.
    pub fn solutions<F>(&self, mut f: F) -> ControlFlow<()>
    where
        F: FnMut(&Self) -> ControlFlow<()>,
    {
        let all: BTreeSet<usize> = (0..self.n).collect();
        let root = self.propagate(all.clone(), all);
        root.search(&mut f)
    }

    fn search<F>(&self, f: &mut F) -> ControlFlow<()>
    where
        F: FnMut(&Self) -> ControlFlow<()>,
    {
        if !self.is_valid() {
            return ControlFlow::Continue(());
        }
        if self.is_solved() {
            return f(self);
        }
        let Some(cell) = self.mrv_cell() else {
            return ControlFlow::Continue(());
        };
        for child in self.branch(cell) {
            child.search(f)?;
        }
        ControlFlow::Continue(())
    }

    /// Returns the first solution found, or `None` if the puzzle is unsatisfiable.
    #[must_use]
    pub fn first_solution(&self) -> Option<Self> {
        let mut found = None;
        let _ = self.solutions(|p| {
            found = Some(p.clone());
            ControlFlow::Break(())
        });
        found
    }

    /// True iff the puzzle has exactly one solution. Stops searching after the second.
    #[must_use]
    pub fn is_uniquely_solvable(&self) -> bool {
        let mut count = 0u32;
        let _ = self.solutions(|_| {
            count += 1;
            if count >= 2 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        count == 1
    }
}

#[cfg(test)]
mod tests {
    use std::ops::ControlFlow;

    use crate::puzzle::{BTreeSet, Cell, Puzzle, Value};
    use crate::test_fixtures::fixtures::make_3x3_puzzle_cages;

    fn bare_fixture() -> Puzzle {
        Puzzle::new(3, make_3x3_puzzle_cages())
    }

    fn propagate_removals(p: &Puzzle, removals: &BTreeSet<(Cell, Value)>) -> Puzzle {
        let (p, changed) = p.cage_constraints(removals);
        let rows = changed.iter().map(|&(r, _)| r).collect();
        let cols = changed.iter().map(|&(_, c)| c).collect();
        p.propagate(rows, cols)
    }

    fn solved_fixture() -> Puzzle {
        propagate_removals(
            &bare_fixture(),
            &BTreeSet::from([((0, 1), 2), ((2, 1), 2), ((1, 0), 2), ((1, 2), 2)]),
        )
    }

    fn invalid_fixture() -> Puzzle {
        propagate_removals(&bare_fixture(), &BTreeSet::from([((1, 1), 2)]))
    }

    #[test]
    fn solved_puzzle_calls_callback_once() {
        let p = solved_fixture();
        assert!(p.is_solved());
        let mut count = 0;
        let _ = p.solutions(|_| {
            count += 1;
            ControlFlow::Continue(())
        });
        assert_eq!(count, 1);
    }

    #[test]
    fn invalid_puzzle_calls_callback_zero_times() {
        let p = invalid_fixture();
        assert!(!p.is_valid());
        let mut count = 0;
        let _ = p.solutions(|_| {
            count += 1;
            ControlFlow::Continue(())
        });
        assert_eq!(count, 0);
    }

    #[test]
    fn fixture_has_exactly_one_solution() {
        let p = bare_fixture();
        let mut solutions = Vec::new();
        let _ = p.solutions(|s| {
            solutions.push(s.clone());
            ControlFlow::Continue(())
        });
        assert_eq!(solutions.len(), 1);
        assert!(solutions[0].is_solved());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn first_solution_returns_some_and_is_solved() {
        let p = bare_fixture();
        let sol = p.first_solution();
        assert!(sol.is_some());
        assert!(sol.unwrap().is_solved());
    }

    #[test]
    fn first_solution_returns_none_for_invalid() {
        let p = invalid_fixture();
        assert!(p.first_solution().is_none());
    }

    #[test]
    fn is_uniquely_solvable_true_for_fixture() {
        assert!(bare_fixture().is_uniquely_solvable());
    }

    #[test]
    fn break_halts_search_after_one_call() {
        let p = bare_fixture();
        let mut count = 0;
        let _ = p.solutions(|_| {
            count += 1;
            ControlFlow::Break(())
        });
        assert_eq!(count, 1);
    }

    #[test]
    fn mrv_cell_none_on_solved() {
        let p = solved_fixture();
        assert!(p.is_solved());
        assert!(p.mrv_cell().is_none());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn mrv_cell_some_on_bare_fixture() {
        let p = bare_fixture();
        let cell = p.mrv_cell().unwrap();
        let cage = p.cages_containing(cell).unwrap();
        let domain = p.state(cage).values(cage).remove(&cell).unwrap_or_default();
        assert!(domain.len() >= 2);
    }

    #[test]
    fn solutions_on_bare_fixture_finds_unique_solution() {
        // Regression: root-propagation step must fire before first branch.
        let mut solutions = Vec::new();
        let _ = bare_fixture().solutions(|s| {
            solutions.push(s.clone());
            ControlFlow::Continue(())
        });
        assert_eq!(solutions.len(), 1);
    }
}
