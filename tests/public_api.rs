//! Integration tests for the crate's public API. These run as an external user would,
//! guarding against accidental visibility regressions.

#![allow(clippy::unwrap_used)]

use kenken::{
    DEFAULT_SIZE_DISTRIBUTION, Index, N, Operation, Puzzle, SizeDistribution, Uniqueness,
    default_op_policy, generate, generate_with,
};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

fn rng(seed: u64) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(seed)
}

#[test]
fn uniqueness_classifies_every_generated_puzzle() {
    let n = 3;
    let trials = 30;
    let mut r = rng(0);
    let mut hist = [0usize; 3];
    for _ in 0..trials {
        let p: Puzzle = generate(n, &mut r).unwrap();
        match p.uniqueness() {
            Uniqueness::None => hist[0] += 1,
            Uniqueness::Unique => hist[1] += 1,
            Uniqueness::Multiple => hist[2] += 1,
        }
    }
    assert_eq!(hist.iter().sum::<usize>(), trials);
}

#[test]
fn solutions_returns_a_count() {
    let mut r = rng(1);
    let total: usize = (0..10)
        .map(|_| generate(3, &mut r).unwrap().solutions())
        .sum();
    assert!(total >= 10);
}

#[test]
fn generate_validates_n_too_small() {
    let mut r = rng(0);
    assert!(generate(0, &mut r).is_err());
}

#[test]
fn generate_validates_n_too_large() {
    let mut r = rng(0);
    assert!(generate(10, &mut r).is_err());
}

#[test]
fn puzzle_n_returns_grid_size() {
    let mut r = rng(0);
    let p = generate(4, &mut r).unwrap();
    assert_eq!(p.n(), 4);
}

#[test]
fn deterministic_with_same_seed() {
    let a = generate(3, &mut rng(42)).unwrap();
    let b = generate(3, &mut rng(42)).unwrap();
    assert_eq!(a.uniqueness(), b.uniqueness());
    assert_eq!(a.solutions(), b.solutions());
}

#[test]
fn generated_puzzles_are_always_solvable() {
    // The Latin square used during construction is itself a valid solution, so every
    // generated puzzle must have ≥ 1 solution.
    for seed in 0..20u64 {
        for n in 2..=4 {
            let p = generate(n, &mut rng(seed)).unwrap();
            assert_ne!(
                p.uniqueness(),
                Uniqueness::None,
                "seed {seed} n={n} produced unsolvable puzzle"
            );
        }
    }
}

#[test]
fn uniqueness_and_solutions_buckets_agree() {
    for seed in 0..30u64 {
        let p = generate(3, &mut rng(seed)).unwrap();
        let (bucket, count) = (p.uniqueness(), p.solutions());
        let consistent = matches!(
            (bucket, count),
            (Uniqueness::None, 0)
                | (Uniqueness::Unique, 1)
                | (Uniqueness::Multiple, 2..=usize::MAX)
        );
        assert!(
            consistent,
            "seed {seed}: bucket {bucket:?} but count {count}"
        );
    }
}

#[test]
fn custom_op_policy_overrides_default() {
    let policy = |values: &[N], n: Index| {
        if values.len() == 2 {
            Operation::Add(values.iter().sum())
        } else {
            default_op_policy(values, n)
        }
    };
    let p = generate_with(4, &mut rng(99), policy, DEFAULT_SIZE_DISTRIBUTION).unwrap();
    assert_ne!(p.uniqueness(), Uniqueness::None);
}

#[test]
fn fixed_size_one_distribution_yields_unique_puzzles() {
    // Fixed(1) cages each pin a single cell to its solved value, so the puzzle is fully
    // determined.
    let p = generate_with(
        4,
        &mut rng(7),
        default_op_policy,
        SizeDistribution::Fixed(1),
    )
    .unwrap();
    assert_eq!(p.uniqueness(), Uniqueness::Unique);
    assert_eq!(p.solutions(), 1);
}

#[test]
fn size_distribution_uniform_is_constructible() {
    let dist = SizeDistribution::Uniform { min: 2, max: 3 };
    let p = generate_with(4, &mut rng(5), default_op_policy, dist).unwrap();
    assert_ne!(p.uniqueness(), Uniqueness::None);
}
