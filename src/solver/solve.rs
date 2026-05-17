use crate::Error;

/// A search state that can be propagated to a fixed point and branched into
/// substates.
pub trait State: Sized {
    /// Propagates constraints to a fixed point; returns `None` to prune this
    /// branch.
    ///
    /// # Errors
    /// Returns [`Error`] if a constraint is applied to a cell outside the grid.
    fn propagate(&self) -> Result<Option<Self>, Error>;
    /// Returns substates to explore.
    fn branch(&self) -> impl Iterator<Item = Self>;
    /// Returns `true` when this state represents a complete solution. The
    /// solver only yields states for which this is `true`.
    fn is_solved(&self) -> bool;
}

/// A depth-first backtracking solver over any [`State`].
///
/// Each call to [`Iterator::next`] returns one solution — a state for which
/// [`State::is_solved`] is `true`. [`crate::Puzzle`] implements [`State`], so
/// `Solver<Puzzle>` enumerates all solutions to a puzzle.
pub struct Solver<S> {
    stack: Vec<S>,
}

impl<S: State> Solver<S> {
    /// Creates a new solver starting from `root`.
    pub fn new(root: S) -> Self {
        Self { stack: vec![root] }
    }
}

impl<S: State + Clone> Iterator for Solver<S> {
    type Item = S;

    fn next(&mut self) -> Option<S> {
        while let Some(state) = self.stack.pop() {
            let Ok(Some(state)) = state.propagate() else {
                continue;
            };
            // is_solved is authoritative: a `State` is free to prune children
            // inside branch(), so an empty branch iterator can mean either
            // "no work left" or "all candidates were infeasible" — only the
            // former is a solution.
            if state.is_solved() {
                return Some(state);
            }
            self.stack.extend(state.branch());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Search state for prime factorization of `n`.
    /// `candidate` is the next factor to try; `factors` accumulates confirmed
    /// factors.
    #[derive(Clone, Debug)]
    struct Factoring {
        remaining: u64,
        candidate: u64,
        factors: Vec<u64>,
    }

    impl Factoring {
        fn new(n: u64) -> Self {
            Self {
                remaining: n,
                candidate: 2,
                factors: vec![],
            }
        }
    }

    impl State for Factoring {
        fn propagate(&self) -> Result<Option<Self>, Error> {
            if self.remaining == 0 {
                return Ok(None);
            }
            let mut remaining = self.remaining;
            let mut factors = self.factors.clone();
            while remaining.is_multiple_of(self.candidate) {
                factors.push(self.candidate);
                remaining /= self.candidate;
            }
            // Any factor of remaining must be > candidate, so remaining itself is prime.
            if (self.candidate + 1) * (self.candidate + 1) > remaining && remaining > 1 {
                factors.push(remaining);
                remaining = 1;
            }
            Ok(Some(Self {
                remaining,
                factors,
                ..*self
            }))
        }

        fn branch(&self) -> impl Iterator<Item = Self> {
            if self.remaining == 1 {
                None
            } else {
                Some(Self {
                    remaining: self.remaining,
                    candidate: self.candidate + 1,
                    factors: self.factors.clone(),
                })
            }
            .into_iter()
        }

        fn is_solved(&self) -> bool {
            self.remaining == 1
        }
    }

    fn factors(n: u64) -> Vec<u64> {
        Solver::new(Factoring::new(n))
            .next()
            .map(|s| s.factors)
            .unwrap_or_default()
    }

    #[test]
    fn prime_has_one_factor() {
        assert_eq!(factors(7), vec![7]);
        assert_eq!(factors(13), vec![13]);
        assert_eq!(factors(97), vec![97]);
    }

    #[test]
    fn prime_squared() {
        assert_eq!(factors(4), vec![2, 2]);
        assert_eq!(factors(9), vec![3, 3]);
        assert_eq!(factors(25), vec![5, 5]);
    }

    #[test]
    fn product_of_two_distinct_primes() {
        assert_eq!(factors(6), vec![2, 3]);
        assert_eq!(factors(15), vec![3, 5]);
        assert_eq!(factors(35), vec![5, 7]);
    }

    #[test]
    fn product_of_three_primes() {
        assert_eq!(factors(30), vec![2, 3, 5]);
        assert_eq!(factors(2310), vec![2, 3, 5, 7, 11]);
    }

    #[test]
    fn power_of_two() {
        assert_eq!(factors(64), vec![2, 2, 2, 2, 2, 2]);
    }

    #[test]
    fn one_has_no_factors() {
        assert_eq!(factors(1), vec![]);
    }

    #[test]
    fn zero_has_no_factors() {
        assert_eq!(factors(0), vec![]);
    }

    #[test]
    fn large_semiprime() {
        // 9_999_991 = 3_163 × 3_163? No — check it's prime.
        // 997 × 1009 = 1_005_973
        assert_eq!(factors(1_005_973), vec![997, 1_009]);
    }
}
