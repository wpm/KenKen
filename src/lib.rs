mod arithmetic;
mod constraints;
mod generation;
mod grid;
mod latin_square;
mod puzzle;
mod regin;
mod shape;
mod solver;
mod tiling;
mod types;

pub use constraints::Operation;
pub use generation::{DEFAULT_SIZE_DISTRIBUTION, default_op_policy, generate, generate_with};
pub use puzzle::{Puzzle, Uniqueness};
pub use tiling::SizeDistribution;
pub use types::{Error, Index, M, N};
