//! KenKen puzzle generator and solver.
//!
//! The primary interface is [`Puzzle`]: construct one with [`Puzzle::new`], add
//! [`Cage`]s via [`Puzzle::insert_cage`], and inspect solutions with
//! [`Puzzle::uniqueness`] and [`Puzzle::solutions`].
//!
//! For direct enumeration of solutions, [`Solver`] is a depth-first
//! backtracking iterator over any type that implements [`State`]; [`Puzzle`]
//! implements [`State`].
//!
//! Puzzles can also be generated randomly via [`generate`] or
//! [`generate_with`].

pub mod constraints;
mod generator;
mod puzzle;
mod solver;
mod types;

mod grid;

pub use constraints::{
    cage::{
        builder::{Cage, Tuple},
        operation::{Operation, Operator},
        operator_tuples,
    },
    tiling::SizeDistribution,
};
pub use generator::generate::{
    DEFAULT_SIZE_DISTRIBUTION, default_op_policy, generate, generate_with,
};
pub use grid::Grid;
pub use puzzle::{NarrowingScore, Puzzle, Uniqueness};
pub use solver::{
    delta::Delta,
    solve::{Solver, State},
};
pub use types::{Cell, Error, Fill};
