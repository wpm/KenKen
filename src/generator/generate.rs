//! Core puzzle generation: assigns operations and targets to cages over a
//! solved Latin square.

use rand::Rng;

use crate::{
    Cage,
    constraints::{
        cage::operation::Operation,
        cover::Cover,
        tiling::{SizeDistribution, Tiling},
    },
    generator::latin_square::generate_latin_square,
    puzzle::Puzzle,
    types::{Error, Index, M, N},
};

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
/// # Panics
/// Panics if `values` is empty. A cage always covers at least one cell, so
/// callers that obtain `values` from a cage's cells will never trigger this.
#[must_use]
#[allow(clippy::panic)]
pub fn default_op_policy(values: &[N], n: Index) -> Operation {
    use Operation::{Add, Divide, Given, Multiply, Subtract};
    match values.len() {
        0 => panic!("default_op_policy called with empty values slice"),
        1 => Given(M::from(values[0])),
        2 => {
            let (hi, lo) = (values[0].max(values[1]), values[0].min(values[1]));
            if hi.is_multiple_of(lo) {
                Divide(M::from(hi / lo))
            } else {
                Subtract(M::from(hi - lo))
            }
        }
        _ => {
            let prod: M = values.iter().map(|&v| M::from(v)).product();
            let area = M::try_from(n * n).unwrap_or(M::MAX);
            if prod <= area {
                Multiply(prod)
            } else {
                Add(values.iter().map(|&v| M::from(v)).sum())
            }
        }
    }
}

/// Generates a random `n`×`n` puzzle using [`default_op_policy`] and
/// [`DEFAULT_SIZE_DISTRIBUTION`].
///
/// # Errors
/// Returns `Error` if `n` is not in `1..=9`.
pub fn generate<R: Rng>(n: Index, rng: &mut R) -> Result<Puzzle, Error> {
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
#[allow(clippy::cast_possible_truncation)]
pub fn generate_with<R: Rng, F>(
    n: Index,
    rng: &mut R,
    op: F,
    sizes: SizeDistribution,
) -> Result<Puzzle, Error>
where
    F: Fn(&[N], Index) -> Operation,
{
    let mut puzzle = Puzzle::new(n)?;
    let latin_square = generate_latin_square(n, rng);
    let tiling = Tiling::greedy(n, &sizes, rng);
    let n_max = n as N;

    for polyomino in tiling.into_polyominos() {
        let values: Vec<N> = polyomino
            .cells()
            .iter()
            .map(|cell| latin_square[cell.row][cell.column])
            .collect();
        let operation = op(&values, n);
        let cage = Cage::new(n_max, polyomino, operation);
        puzzle = puzzle.insert_cage(cage)?;
    }
    Ok(puzzle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_op_policy_one_cell_is_given() {
        assert_eq!(default_op_policy(&[3], 4), Operation::Given(3));
    }

    #[test]
    fn default_op_policy_two_cells_divisible_is_divide() {
        assert_eq!(default_op_policy(&[2, 6], 6), Operation::Divide(3));
    }

    #[test]
    fn default_op_policy_two_cells_not_divisible_is_subtract() {
        assert_eq!(default_op_policy(&[2, 5], 6), Operation::Subtract(3));
    }

    #[test]
    fn default_op_policy_three_cells_product_within_n_squared_is_multiply() {
        // n²=16; product 1·2·3 = 6 ≤ 16
        assert_eq!(default_op_policy(&[1, 2, 3], 4), Operation::Multiply(6));
    }

    #[test]
    fn default_op_policy_three_cells_product_above_n_squared_is_add() {
        // n²=16; product 3·4·4 = 48 > 16
        assert_eq!(default_op_policy(&[3, 4, 4], 4), Operation::Add(11));
    }

    #[test]
    #[should_panic(expected = "default_op_policy called with empty values slice")]
    fn default_op_policy_empty_panics() {
        let _ = default_op_policy(&[], 4);
    }
}
