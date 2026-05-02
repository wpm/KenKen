#![allow(dead_code)]

pub trait State: Sized {
    fn propagate(self) -> Option<Self>;
    fn branch(self) -> impl Iterator<Item = Self>;
}

#[must_use]
pub struct Solver<S> {
    stack: Vec<S>,
}

impl<S: State> Solver<S> {
    pub fn new(root: S) -> Self {
        Self { stack: vec![root] }
    }
}

impl<S: State + Clone> Iterator for Solver<S> {
    type Item = S;

    fn next(&mut self) -> Option<S> {
        while let Some(state) = self.stack.pop() {
            if let Some(state) = state.propagate() {
                let branches: Vec<S> = state.clone().branch().collect();
                if branches.is_empty() {
                    return Some(state);
                }
                self.stack.extend(branches);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Search state for prime factorization of `n`.
    /// `candidate` is the next factor to try; `factors` accumulates confirmed factors.
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
        /// Divide out all copies of `candidate` from `remaining`.
        /// Also folds in the final prime when candidate² > remaining.
        /// Returns `None` if the state is already invalid (remaining == 0).
        fn propagate(mut self) -> Option<Self> {
            if self.remaining == 0 {
                return None;
            }
            while self.remaining.is_multiple_of(self.candidate) {
                self.factors.push(self.candidate);
                self.remaining /= self.candidate;
            }
            // If no larger candidate can divide remaining, it must be prime.
            if (self.candidate + 1) * (self.candidate + 1) > self.remaining && self.remaining > 1 {
                self.factors.push(self.remaining);
                self.remaining = 1;
            }
            Some(self)
        }

        /// Try the next candidate factor.
        fn branch(self) -> impl Iterator<Item = Self> {
            if self.remaining == 1 {
                return itertools::Either::Left(std::iter::empty());
            }
            itertools::Either::Right(std::iter::once(Self {
                candidate: self.candidate + 1,
                ..self
            }))
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
    fn large_semiprime() {
        // 9_999_991 = 3_163 × 3_163? No — check it's prime.
        // 997 × 1009 = 1_005_973
        assert_eq!(factors(1_005_973), vec![997, 1_009]);
    }
}
