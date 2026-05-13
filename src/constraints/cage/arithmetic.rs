use crate::constraints::cage::builder::Tuple;
use crate::types::{M, N};

/// Returns an iterator over all non-decreasing `k`-tuples with values in `1..=n` that sum to `s`.
pub fn addition_multisets(n: N, k: usize, s: N) -> impl Iterator<Item = Tuple> {
    simplex_multisets(n, k, |acc, i| acc + M::from(i), M::from(s))
}

/// Returns all 2-element tuples `[i, j]` with `j - i == d` and `1 <= i < j <= max`.
pub fn subtraction_multisets(max: N, d: N) -> impl Iterator<Item = Tuple> {
    (1..=max.saturating_sub(d)).map(move |i| vec![i, i + d])
}

/// Returns an iterator over all non-decreasing `k`-tuples with values in `1..=n` whose product is `s`.
pub fn multiplication_multisets(n: N, k: usize, s: M) -> impl Iterator<Item = Tuple> {
    simplex_multisets(n, k, |acc, i| acc * M::from(i), s)
}

/// Returns all 2-element tuples `[i, j]` with `j / i == q` and `1 <= i < j <= max`.
pub fn division_multisets(max: N, q: N) -> impl Iterator<Item = Tuple> {
    (1..=max / q).map(move |i| vec![i, i * q])
}

/// Returns an iterator over all non-decreasing `tuple_size`-tuples with values in `1..=n` where
/// applying `f` across the tuple (left fold) equals `s`.
///
/// `f` folds an `M` accumulator over each `N` element, so products that exceed `u8::MAX` are
/// handled correctly. Partial tuples whose accumulated value already exceeds `s` are pruned,
/// since extending them can only increase it. Complete tuples are yielded only when their
/// value equals `s`.
#[allow(clippy::many_single_char_names)]
fn simplex_multisets(
    n: N,
    tuple_size: usize,
    f: impl Fn(M, N) -> M + Copy + 'static,
    s: M,
) -> Box<dyn Iterator<Item = Tuple>> {
    fn recurse(
        n: N,
        tuple_size: usize,
        k: usize,
        f: impl Fn(M, N) -> M + Copy + 'static,
        s: M,
    ) -> Box<dyn Iterator<Item = Tuple>> {
        if k == 0 {
            return Box::new(std::iter::once(vec![]));
        }
        Box::new(recurse(n, tuple_size, k - 1, f, s).flat_map(move |t| {
            let last = t.last().copied().unwrap_or(1);
            (last..=n)
                .map(move |i| {
                    let mut t = t.clone();
                    t.push(i);
                    t
                })
                .filter(move |t| {
                    let v = apply(f, t);
                    if t.len() == tuple_size {
                        v == s
                    } else {
                        // Prune partials whose accumulated value already exceeds s; extending them can only increase it.
                        v <= s
                    }
                })
                .collect::<Vec<_>>()
        }))
    }
    recurse(n, tuple_size, tuple_size, f, s)
}

/// Reduces `t` by left-folding `f` over its elements, starting from the first element widened
/// to `M`. Panics if `t` is empty.
#[allow(clippy::panic)]
fn apply(f: impl Fn(M, N) -> M, t: &Tuple) -> M {
    let Some((&t_0, rest)) = t.split_first() else {
        panic!("tuple must have at least one element");
    };
    rest.iter().fold(M::from(t_0), |acc, &i| f(acc, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtraction_multisets_target_1_min_1() {
        assert_eq!(
            subtraction_multisets(4, 1).collect::<Vec<_>>(),
            vec![vec![1, 2], vec![2, 3], vec![3, 4]]
        );
    }

    #[test]
    fn subtraction_multisets_target_2_min_1() {
        assert_eq!(
            subtraction_multisets(4, 2).collect::<Vec<_>>(),
            vec![vec![1, 3], vec![2, 4]]
        );
    }

    #[test]
    fn subtraction_multisets_no_results() {
        assert_eq!(
            subtraction_multisets(3, 5).collect::<Vec<_>>(),
            Vec::<Tuple>::new()
        );
    }

    #[test]
    fn division_multisets_target_2_min_1() {
        assert_eq!(
            division_multisets(4, 2).collect::<Vec<_>>(),
            vec![vec![1, 2], vec![2, 4]]
        );
    }

    #[test]
    fn division_multisets_target_3_min_1() {
        assert_eq!(
            division_multisets(6, 3).collect::<Vec<_>>(),
            vec![vec![1, 3], vec![2, 6]]
        );
    }

    #[test]
    fn division_multisets_no_results() {
        assert_eq!(
            division_multisets(5, 7).collect::<Vec<_>>(),
            Vec::<Tuple>::new()
        );
    }

    #[test]
    fn simplex_multisets_filters_by_sum() {
        // k=2, n=4, sum=5: pairs are [1,4], [2,3]
        assert_eq!(
            addition_multisets(4, 2, 5).collect::<Vec<_>>(),
            vec![vec![1, 4], vec![2, 3]]
        );
    }

    #[test]
    fn simplex_multisets_filters_by_product() {
        // k=2, n=6, product=6: pairs are [1,6], [2,3]
        assert_eq!(
            multiplication_multisets(6, 2, 6).collect::<Vec<_>>(),
            vec![vec![1, 6], vec![2, 3]]
        );
    }

    #[test]
    fn addition_multisets_k3() {
        // n=4, k=3, s=6: [1,1,4], [1,2,3], [2,2,2]
        assert_eq!(
            addition_multisets(4, 3, 6).collect::<Vec<_>>(),
            vec![vec![1, 1, 4], vec![1, 2, 3], vec![2, 2, 2]]
        );
    }

    #[test]
    fn addition_multisets_no_results() {
        // s=1 with k=2 is impossible since min sum is 1+1=2
        assert_eq!(
            addition_multisets(4, 2, 1).collect::<Vec<_>>(),
            Vec::<Tuple>::new()
        );
    }

    #[test]
    fn multiplication_multisets_k2() {
        // n=6, k=2, s=6: [1,6], [2,3]
        assert_eq!(
            multiplication_multisets(6, 2, 6).collect::<Vec<_>>(),
            vec![vec![1, 6], vec![2, 3]]
        );
    }

    #[test]
    fn multiplication_multisets_k3() {
        // n=4, k=3, s=8: [1,2,4], [2,2,2]
        assert_eq!(
            multiplication_multisets(4, 3, 8).collect::<Vec<_>>(),
            vec![vec![1, 2, 4], vec![2, 2, 2]]
        );
    }

    #[test]
    fn multiplication_multisets_no_results() {
        // s=7 is prime, so no factorization into 3 values in 1..=4
        assert_eq!(
            multiplication_multisets(4, 3, 7).collect::<Vec<_>>(),
            Vec::<Tuple>::new()
        );
    }

    #[test]
    fn multiplication_multisets_large_product() {
        // 9^2 = 81 fits in N, but 9^3 = 729 does not — requires M
        assert_eq!(
            multiplication_multisets(9, 3, 729).collect::<Vec<_>>(),
            vec![vec![9, 9, 9]]
        );
    }
}
