#![allow(dead_code)]

use crate::constraints::Operation;
use crate::types::{M, N};
use itertools::Itertools;
use std::collections::BTreeSet;

/// Returns all ordered tuples of `k` values from `1..=max` satisfying `operation`.
#[must_use]
pub fn cage_tuples(n: N, cage_size: usize, operation: &Operation) -> Vec<Vec<N>> {
    let multisets: Vec<Vec<N>> = match operation {
        Operation::Add(target) => addition_multisets(*target, cage_size, n),
        Operation::Multiply(target) => multiplication_multisets(*target, cage_size, n),
        Operation::Subtract(target) => subtraction_multisets(*target, n),
        Operation::Divide(target) => division_multisets(*target, n),
        Operation::Given(value) => vec![vec![*value]],
    };
    permute(multisets, cage_size)
}

/// Expands a list of multisets into distinct ordered permutations of length `k`.
fn permute(multisets: Vec<Vec<N>>, k: usize) -> Vec<Vec<N>> {
    let mut result: BTreeSet<Vec<N>> = BTreeSet::new();
    for m in multisets {
        for p in m.into_iter().permutations(k) {
            result.insert(p);
        }
    }
    result.into_iter().collect()
}

/// Returns all multisets of `cage_size` values from `1..=max` that sum to `target`.
fn addition_multisets(target: N, cage_size: usize, max: N) -> Vec<Vec<N>> {
    fn recurse(target: N, k: usize, min: N, max: N, current: &mut Vec<N>, out: &mut Vec<Vec<N>>) {
        if k == 0 {
            if target == 0 {
                out.push(current.clone());
            }
            return;
        }
        for v in min..=max {
            if v > target {
                break;
            }
            current.push(v);
            recurse(target - v, k - 1, v, max, current, out);
            current.pop();
        }
    }
    let mut out = Vec::new();
    recurse(target, cage_size, 1, max, &mut Vec::new(), &mut out);
    out
}

/// Returns all multisets of `cage_size` values from `1..=max` whose product equals `target`.
fn multiplication_multisets(target: M, cage_size: usize, max: N) -> Vec<Vec<N>> {
    fn recurse(target: M, k: usize, min: N, max: N, current: &mut Vec<N>, out: &mut Vec<Vec<N>>) {
        if k == 0 {
            if target == 1 {
                out.push(current.clone());
            }
            return;
        }
        for v in min..=max {
            if !target.is_multiple_of(M::from(v)) {
                continue;
            }
            current.push(v);
            recurse(target / M::from(v), k - 1, v, max, current, out);
            current.pop();
        }
    }
    let mut out = Vec::new();
    recurse(target, cage_size, 1, max, &mut Vec::new(), &mut out);
    out
}

/// Subtraction cages always have exactly 2 cells: values {a, b} where |a - b| = target.
fn subtraction_multisets(target: N, max: N) -> Vec<Vec<N>> {
    (1..=max)
        .filter_map(|a| {
            let b = a + target;
            if b <= max { Some(vec![a, b]) } else { None }
        })
        .collect()
}

/// Division cages always have exactly 2 cells: values {a, b} where max(a,b)/min(a,b) = target.
fn division_multisets(target: N, max: N) -> Vec<Vec<N>> {
    (1..=max)
        .filter_map(|a| {
            let b = a * target;
            if b <= max { Some(vec![a, b]) } else { None }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::Operation;

    #[test]
    fn add_two_cells() {
        let tuples = cage_tuples(4, 2, &Operation::Add(3));
        assert!(tuples.contains(&vec![1, 2]));
        assert!(tuples.contains(&vec![2, 1]));
        assert_eq!(tuples.len(), 2);
    }

    #[test]
    fn add_repeated_values_deduped() {
        let tuples = cage_tuples(3, 2, &Operation::Add(6));
        assert_eq!(tuples, vec![vec![3, 3]]);
    }

    #[test]
    fn add_no_solution() {
        let tuples = cage_tuples(4, 2, &Operation::Add(1));
        assert!(tuples.is_empty());
    }

    #[test]
    fn multiply_two_cells() {
        let tuples = cage_tuples(4, 2, &Operation::Multiply(6));
        assert!(tuples.contains(&vec![2, 3]));
        assert!(tuples.contains(&vec![3, 2]));
    }

    #[test]
    fn subtract_two_cells() {
        let tuples = cage_tuples(4, 2, &Operation::Subtract(1));
        assert!(tuples.contains(&vec![1, 2]));
        assert!(tuples.contains(&vec![2, 1]));
        assert!(tuples.contains(&vec![2, 3]));
        assert!(tuples.contains(&vec![3, 2]));
    }

    #[test]
    fn divide_two_cells() {
        let tuples = cage_tuples(4, 2, &Operation::Divide(2));
        assert!(tuples.contains(&vec![1, 2]));
        assert!(tuples.contains(&vec![2, 1]));
        assert!(tuples.contains(&vec![2, 4]));
        assert!(tuples.contains(&vec![4, 2]));
    }

    #[test]
    fn given_single_cell() {
        let tuples = cage_tuples(6, 1, &Operation::Given(5));
        assert_eq!(tuples.len(), 1);
        assert_eq!(tuples[0], vec![5]);
    }
}
