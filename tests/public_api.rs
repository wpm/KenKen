//! Integration tests for the crate's public API. These run as an external user would,
//! guarding against accidental visibility regressions.

#![allow(clippy::unwrap_used)]

use kenken::{
    Cage, Cell, DEFAULT_SIZE_DISTRIBUTION, Delta, Grid, Index, N, NarrowingScore, Operation,
    Polyomino, Puzzle, SizeDistribution, Uniqueness, Values, default_op_policy, generate,
    generate_with,
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
fn puzzle_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Puzzle>();
}

#[test]
fn solutions_at_most_caps_count() {
    let mut r = rng(1);
    let p = generate(3, &mut r).unwrap();
    let total = p.solutions();
    assert_eq!(p.solutions_at_most(0), 0);
    assert_eq!(p.solutions_at_most(usize::MAX), total);
    let cap = total.saturating_add(1);
    assert_eq!(p.solutions_at_most(cap), total);
    if total > 0 {
        assert_eq!(p.solutions_at_most(1), 1);
    }
}

#[test]
fn size_distribution_uniform_is_constructible() {
    let dist = SizeDistribution::Uniform { min: 2, max: 3 };
    let p = generate_with(4, &mut rng(5), default_op_policy, dist).unwrap();
    assert_ne!(p.uniqueness(), Uniqueness::None);
}

fn given_cage(row: Index, column: Index, value: N) -> Cage {
    let cell = Cell::new(row, column);
    Cage::new(value, Polyomino::new(&[cell]), Operation::Given(value))
}

#[test]
fn puzzle_insert_and_remove_cage_are_public() {
    let cage = given_cage(0, 0, 1);
    let poly = cage.polyomino().clone();
    let p = Puzzle::new(2).unwrap().insert_cage(cage).unwrap();
    assert!(p.is_covered(Cell::new(0, 0)));
    let p = p.remove_cage(&poly);
    assert!(!p.is_covered(Cell::new(0, 0)));
}

#[test]
fn grid_set_is_public_and_round_trips() {
    let g: Grid = Grid::new(3).unwrap();
    let cell = Cell::new(1, 2);
    let one = Values::new([1]);
    let g = g.set(&cell, one);
    assert_eq!(g.get(&cell).unwrap(), one);
}

#[test]
fn cage_inherent_accessors_are_reachable() {
    let cage = given_cage(0, 0, 1);
    assert_eq!(cage.operation(), Operation::Given(1));
    assert_eq!(cage.cells(), &[Cell::new(0, 0)]);
    assert_eq!(cage.tuples(), &[vec![1u8]]);
}

#[test]
fn puzzle_grid_and_candidates_expose_state() {
    let p = Puzzle::new(2)
        .unwrap()
        .insert_cage(given_cage(0, 0, 1))
        .unwrap();
    let grid: &Grid = p.grid();
    assert_eq!(grid.n(), 2);
    let v: Values = p.candidates(Cell::new(0, 1)).unwrap();
    assert_eq!(v, Values::full(2));
}

#[test]
fn puzzle_cells_visits_every_grid_cell() {
    let p = Puzzle::new(2).unwrap();
    assert_eq!(p.cells().count(), 4);
}

#[test]
fn fresh_puzzle_has_no_singletons_or_empty_cells() {
    // Fresh 2×2 has every cell containing {1, 2}: no singletons and no empties.
    let p = Puzzle::new(2).unwrap();
    assert_eq!(p.singleton_cells().count(), 0);
    assert_eq!(p.empty_cells().count(), 0);
}

#[test]
fn singleton_grid_yields_its_only_cell() {
    // 1×1: the lone cell holds {1}, which is a singleton by construction.
    let p = Puzzle::new(1).unwrap();
    let singletons: Vec<(Cell, N)> = p.singleton_cells().collect();
    assert_eq!(singletons, vec![(Cell::new(0, 0), 1)]);
}

#[test]
fn polyomino_extend_and_without_are_public() {
    let p = Polyomino::new(&[Cell::new(0, 0), Cell::new(0, 1)]);
    let extended = p.extend(Cell::new(0, 2)).unwrap();
    assert_eq!(extended.as_slice().len(), 3);
    let shrunk = extended.without(Cell::new(0, 2)).unwrap();
    assert_eq!(shrunk, p);
}

#[test]
fn puzzle_cage_accessors_are_reachable() {
    let p = Puzzle::new(2)
        .unwrap()
        .insert_cage(given_cage(0, 0, 1))
        .unwrap()
        .insert_cage(given_cage(1, 1, 2))
        .unwrap();
    assert_eq!(p.cages().count(), 2);
    let cage_at = p.cage_at(Cell::new(0, 0)).unwrap();
    assert_eq!(cage_at.operation(), Operation::Given(1));
    assert!(p.is_covered(Cell::new(1, 1)));
    assert!(!p.is_covered(Cell::new(0, 1)));
}

#[test]
fn rank_tuples_for_cage_is_public_and_returns_scored_entries() {
    // Tuples (1,2) and (2,1) tie on reduction; lex tiebreak orders (1,2) first.
    let cells = [Cell::new(0, 0), Cell::new(0, 1)];
    let cage = Cage::new(4, Polyomino::new(&cells), Operation::Add(3));
    let p = Puzzle::new(4).unwrap().insert_cage(cage.clone()).unwrap();
    let ranked: Vec<(Vec<N>, Puzzle, NarrowingScore)> = p.rank_tuples_for_cage(&cage).unwrap();
    let tuples: Vec<Vec<N>> = ranked.iter().map(|(t, _, _)| t.clone()).collect();
    assert_eq!(tuples, vec![vec![1, 2], vec![2, 1]]);
    assert!(ranked.iter().all(|(_, post, _)| post.is_valid()));
    let score: NarrowingScore = ranked[0].2;
    assert!(score.total_reduction > 0);
}

#[test]
fn delta_narrow_widen_and_propagate_fully_are_public() {
    // Identity delta is a no-op after propagation, regardless of starting state.
    let p = Puzzle::new(2).unwrap();
    let identity = Delta::identity(2).unwrap();
    let unchanged = p.narrow(&identity);
    assert!(unchanged.is_valid());

    // Pinning (0,0)={1} on a 2×2 grid forces a unique completion via narrow + propagate.
    let pinning = Delta::identity(2)
        .unwrap()
        .set(Cell::new(0, 0), Values::new([1]));
    let pinned = Puzzle::new(2).unwrap().narrow(&pinning);
    assert!(pinned.is_valid());
    assert_eq!(
        pinned.candidates(Cell::new(1, 1)).unwrap(),
        Values::new([1]),
    );

    // Emptying a cell via narrow yields an invalid puzzle.
    let emptying = Delta::identity(2)
        .unwrap()
        .set(Cell::new(0, 0), Values::new([]));
    let invalid = Puzzle::new(2).unwrap().narrow(&emptying);
    assert!(!invalid.is_valid());

    // Widen with the identity over a fully-propagated puzzle re-narrows back to the
    // same fixed point.
    let solved = Puzzle::new(2)
        .unwrap()
        .insert_cage(given_cage(0, 0, 1))
        .unwrap()
        .propagate_fully();
    let rewidened = solved.widen(&identity);
    assert!(rewidened.is_valid());
    assert_eq!(
        rewidened.candidates(Cell::new(1, 1)).unwrap(),
        Values::new([1]),
    );
}
