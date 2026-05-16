//! Core puzzle generation: assigns operations and targets to cages over a
//! solved Latin square.

use std::collections::HashSet;

use rand::{Rng, RngExt};

use crate::{
    Cell, Polyomino,
    constraints::{
        Cover,
        cage::{Cage, operation::Operation},
    },
    generator::latin_square::generate_latin_square,
    puzzle::Puzzle,
    types::{Error, Index, M, N},
};
#[derive(Debug, Clone, Copy)]
pub enum SizeDistribution {
    /// Every polyomino has the same target size.
    Fixed(usize),
    /// Target size sampled uniformly from `min..=max`.
    Uniform {
        /// Smallest allowed cage size.
        min: usize,
        /// Largest allowed cage size.
        max: usize,
    },
}

impl SizeDistribution {
    fn sample<R: Rng>(self, rng: &mut R) -> usize {
        match self {
            Self::Fixed(s) => s,
            Self::Uniform { min, max } => rng.random_range(min..=max),
        }
    }
}

/// Default size distribution used by [`generate`]: cage sizes drawn uniformly
/// from `1..=4`.
pub const DEFAULT_SIZE_DISTRIBUTION: SizeDistribution =
    SizeDistribution::Uniform { min: 1, max: 4 };

/// Default policy mapping a cage's solved-grid values to an [`Operation`].
///
/// - 1 cell: [`Operation::Given`].
/// - 2 cells: [`Operation::Divide`] when divisible, otherwise [`Operation::Subtract`].
/// - 3+ cells: [`Operation::Multiply`] when the product fits in `n²`, otherwise [`Operation::Add`].
///
/// # Errors
/// Returns [`Error::EmptyOpPolicyValues`] if `values` is empty. A cage always
/// covers at least one cell, so callers that obtain `values` from a cage's
/// cells will never trigger this.
pub fn default_op_policy(values: &[N], n: Index) -> Result<Operation, Error> {
    use Operation::{Add, Divide, Given, Multiply, Subtract};
    match values.len() {
        0 => Err(Error::EmptyOpPolicyValues),
        1 => Ok(Given(M::from(values[0]))),
        2 => {
            let (hi, lo) = (values[0].max(values[1]), values[0].min(values[1]));
            if hi.is_multiple_of(lo) {
                Ok(Divide(M::from(hi / lo)))
            } else {
                Ok(Subtract(M::from(hi - lo)))
            }
        }
        _ => {
            let prod: M = values.iter().map(|&v| M::from(v)).product();
            let area = M::try_from(n * n).unwrap_or(M::MAX);
            if prod <= area {
                Ok(Multiply(prod))
            } else {
                Ok(Add(values.iter().map(|&v| M::from(v)).sum()))
            }
        }
    }
}

/// Generates a random `n`×`n` puzzle using [`default_op_policy`] and
/// [`DEFAULT_SIZE_DISTRIBUTION`].
///
/// # Errors
/// Returns `Error` if `n` is not in `1..=9`.
pub fn generate<R: Rng>(n: Index, rng: &mut R) -> Result<Option<Puzzle>, Error> {
    generate_with(n, rng, default_op_policy, DEFAULT_SIZE_DISTRIBUTION)
}

/// Generates a random `n`×`n` puzzle with caller-supplied op policy and
/// cage-size distribution.
///
/// The pipeline is:
/// 1. Sample a uniformly random Latin square as the puzzle's solution.
/// 2. Tile the grid with random polyominos sized by `sizes`.
/// 3. For each polyomino, look up the Latin-square values at its cells (in row-major sorted order)
///    and pass them to `op` to choose the cage's operation.
///
/// # Errors
/// Returns `Error` if `n` is not in `1..=9`. The `?` on `insert_cage` is
/// structurally unreachable because the tiling's polyominos are disjoint, but
/// is kept rather than panicking to avoid load-bearing assertions inside the
/// generator.
///
/// # Panics
/// Panics if propagation after inserting a cage returns `None` (no solution
/// exists), which is structurally unreachable when the tiling is valid.
#[allow(clippy::cast_possible_truncation)]
pub fn generate_with<R: Rng, F>(
    n: Index,
    rng: &mut R,
    op: F,
    sizes: SizeDistribution,
) -> Result<Option<Puzzle>, Error>
where
    F: Fn(&[N], Index) -> Result<Operation, Error>,
{
    let mut puzzle = Puzzle::new(n)?;
    let latin_square = generate_latin_square(n, rng);
    let tiling = greedy(n, &sizes, rng)?;
    let n_max = n as N;

    for polyomino in tiling {
        let values: Vec<N> = polyomino
            .cells()
            .map(|cell| latin_square[cell.row][cell.column])
            .collect();
        let operation = op(&values, n)?;
        let cage = Cage::new(n_max, polyomino, operation);
        puzzle = puzzle
            .insert(cage)?
            .unwrap_or_else(|| unreachable!("disjoint tiling cannot produce a contradiction"));
    }
    Ok(Option::from(puzzle))
}

/// Builds a tiling that fully covers an `n`×`n` grid by greedy growth.
///
/// Repeatedly seeds a random uncovered cell, grows it by absorbing random
/// edge-connected uncovered cells until the target size sampled from
/// `dist` is reached or no candidates remain, then starts a new
/// polyomino.
pub fn greedy<R: Rng>(
    n: usize,
    dist: &SizeDistribution,
    rng: &mut R,
) -> Result<Vec<Polyomino>, Error> {
    let mut tiling = Vec::new();
    let mut covered: HashSet<Cell> = HashSet::with_capacity(n * n);

    while covered.len() < n * n {
        let uncovered: Vec<Cell> = (0..n)
            .flat_map(|r| (0..n).map(move |c| Cell::new(r, c)))
            .filter(|c| !covered.contains(c))
            .collect();
        let seed = uncovered[rng.random_range(0..uncovered.len())];
        let target_size = dist.sample(rng);

        let mut cells: HashSet<Cell> = HashSet::new();
        cells.insert(seed);
        // Frontier may contain duplicates; dedup happens on pop via the cells/covered
        // checks.
        let mut frontier: Vec<Cell> = grid_neighbors(seed, n)
            .filter(|c| !covered.contains(c))
            .collect();

        while cells.len() < target_size && !frontier.is_empty() {
            let pick_idx = rng.random_range(0..frontier.len());
            let pick = frontier.swap_remove(pick_idx);
            if !cells.insert(pick) {
                continue;
            }
            for neighbor in grid_neighbors(pick, n) {
                if !covered.contains(&neighbor) && !cells.contains(&neighbor) {
                    frontier.push(neighbor);
                }
            }
        }

        for c in &cells {
            covered.insert(*c);
        }
        let cells: Vec<Cell> = cells.into_iter().collect();
        // `cells` is non-empty (the seed is always inserted) and
        // edge-connected (grown only via `grid_neighbors`), so the
        // validating `Polyomino::new` would always succeed here.
        tiling.push(Polyomino::from_cells(&cells)?);
    }

    Ok(tiling)
}

/// In-bounds 4-neighbors of `cell` in an `n`×`n` grid.
fn grid_neighbors(cell: Cell, n: usize) -> impl Iterator<Item = Cell> {
    cell.neighbors_4()
        .filter(move |c| c.row < n && c.column < n)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    #[test]
    fn default_op_policy_one_cell_is_given() {
        assert_eq!(default_op_policy(&[3], 4).unwrap(), Operation::Given(3));
    }

    #[test]
    fn default_op_policy_two_cells_divisible_is_divide() {
        assert_eq!(default_op_policy(&[2, 6], 6).unwrap(), Operation::Divide(3));
    }

    #[test]
    fn default_op_policy_two_cells_not_divisible_is_subtract() {
        assert_eq!(
            default_op_policy(&[2, 5], 6).unwrap(),
            Operation::Subtract(3)
        );
    }

    #[test]
    fn default_op_policy_three_cells_product_within_n_squared_is_multiply() {
        // n²=16; product 1·2·3 = 6 ≤ 16
        assert_eq!(
            default_op_policy(&[1, 2, 3], 4).unwrap(),
            Operation::Multiply(6)
        );
    }

    #[test]
    fn default_op_policy_three_cells_product_above_n_squared_is_add() {
        // n²=16; product 3·4·4 = 48 > 16
        assert_eq!(
            default_op_policy(&[3, 4, 4], 4).unwrap(),
            Operation::Add(11)
        );
    }

    #[test]
    fn default_op_policy_empty_returns_err() {
        assert!(matches!(
            default_op_policy(&[], 4),
            Err(Error::EmptyOpPolicyValues)
        ));
    }

    #[test]
    fn size_distribution_fixed_always_returns_fixed_size() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
        let dist = SizeDistribution::Fixed(3);
        for _ in 0..10 {
            assert_eq!(dist.sample(&mut rng), 3);
        }
    }

    #[test]
    fn size_distribution_uniform_samples_within_range() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let dist = SizeDistribution::Uniform { min: 2, max: 4 };
        for _ in 0..50 {
            let s = dist.sample(&mut rng);
            assert!((2..=4).contains(&s));
        }
    }

    #[test]
    fn greedy_covers_all_cells() {
        // Run many seeds across different distributions to maximize branch coverage.
        for seed in 0u64..200 {
            let dist = if seed % 2 == 0 {
                SizeDistribution::Fixed(3)
            } else {
                SizeDistribution::Uniform { min: 1, max: 4 }
            };
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
            let n = if seed % 3 == 0 { 4 } else { 3 };
            let tiling = greedy(n, &dist, &mut rng).unwrap();
            let covered: std::collections::HashSet<crate::Cell> =
                tiling.iter().flat_map(Cover::cells).collect();
            assert_eq!(covered.len(), n * n);
        }
    }

    #[test]
    fn generate_returns_a_puzzle() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(7);
        assert!(generate(4, &mut rng).unwrap().is_some());
    }

    #[test]
    fn generate_invalid_n_returns_err() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
        assert!(generate(0, &mut rng).is_err());
        assert!(generate(10, &mut rng).is_err());
    }
}
