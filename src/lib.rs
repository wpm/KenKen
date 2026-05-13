//! KenKen puzzle generator and solver.
//!
//! The primary interface is [`Puzzle`]: construct one with [`Puzzle::new`], add [`Cage`]s via
//! [`Puzzle::insert_cage`], and inspect solutions with [`Puzzle::uniqueness`] and
//! [`Puzzle::solutions`].
//!
//! Puzzles can also be generated randomly via [`generate()`] or [`generate_with`].

mod constraints;
mod generation;
mod geometry;
mod puzzle;
mod solver;
mod types;

pub use constraints::cage::builder::{Cage, Tuple};
pub use constraints::cage::operation::{Operation, Operator};
pub use constraints::cage::operator_tuples;
pub use generation::generate::generate;
pub use generation::generate::{DEFAULT_SIZE_DISTRIBUTION, default_op_policy, generate_with};
pub use geometry::grid::Grid;
pub use geometry::shape::Polyomino;
pub use geometry::tiling::SizeDistribution;
pub use puzzle::{NarrowingScore, Puzzle, Uniqueness};
pub use solver::delta::Delta;
pub use types::{Cell, Error, Index, M, N, Values};
