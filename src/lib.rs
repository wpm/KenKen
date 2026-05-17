//! KenKen puzzle generator and solver.
//!
//! [`Puzzle`] is the entry point for everything the crate does:
//! - Build an empty board with [`Puzzle::new_empty`] and add constraints with
//!   [`Puzzle::insert`], or bulk-construct with [`Puzzle::with_cages`].
//! - Enumerate solutions with [`Puzzle::solve`].
//! - Generate a random puzzle with [`Puzzle::generate`] (or
//!   [`Puzzle::generate_with`] for a custom operation policy and cage-size
//!   distribution).

#![allow(clippy::must_use_candidate, clippy::return_self_not_must_use)]

mod constraints;
mod generator;
mod grid;
mod puzzle;
mod solver;
mod types;

pub use constraints::{
    cage::{
        Cage, Tuple,
        operation::{Operation, Operator},
    },
    polyomino::Polyomino,
};
pub use generator::generate::SizeDistribution;
pub use puzzle::Puzzle;
pub use types::{Cell, Error};

pub(crate) use constraints::Cover;
pub(crate) use grid::Grid;
pub(crate) use types::Fill;
