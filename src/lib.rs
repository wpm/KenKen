pub(crate) mod constraints;
mod generation;
pub(crate) mod geometry;
mod puzzle;
pub(crate) mod solver;
mod types;

pub use constraints::cage::builder::{Cage, Tuple};
pub use constraints::cage::operation::{Operation, Operator};
pub use constraints::cage::operator_tuples;
pub use generation::generate;
pub use generation::generate::{DEFAULT_SIZE_DISTRIBUTION, default_op_policy, generate_with};
pub use geometry::grid::Grid;
pub use geometry::shape::Polyomino;
pub use geometry::tiling::SizeDistribution;
pub use puzzle::{NarrowingScore, Puzzle, Uniqueness};
pub use solver::delta::Delta;
pub use types::{Cell, Error, Index, M, N, Values};
