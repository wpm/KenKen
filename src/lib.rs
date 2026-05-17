//! KenKen puzzle generator and solver.
//!
//! The primary interface is [`Puzzle`]: construct one with [`Puzzle::new_empty`] and
//! add [`Cage`]s via [`Puzzle::insert`].
//!
//! For direct enumeration of solutions, [`Solver`] is a depth-first
//! backtracking iterator over any type that implements [`State`]; [`Puzzle`]
//! implements [`State`].
//!
//! Puzzles can also be generated randomly via [`generate`] or
//! [`generate_with`].

#![allow(clippy::must_use_candidate, clippy::return_self_not_must_use)]

pub mod constraints;
mod generator;
mod puzzle;
mod solver;
mod types;

mod grid;

pub use constraints::{
    Cover,
    cage::{
        Cage, Tuple,
        operation::{Operation, Operator},
    },
    polyomino::Polyomino,
};
pub use generator::generate::{
    SizeDistribution, default_op_policy, generate, generate_with,
};
pub use grid::Grid;
pub use puzzle::Puzzle;
pub use solver::solve::{Solver, State};
pub use types::{Cell, Error, Fill};
