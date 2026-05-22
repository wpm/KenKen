//! KenKen puzzle generator and solver.
//!
//! [`Puzzle`] is the entry point for everything the crate does:
//! - Build an empty board with [`Puzzle::new`] and add constraints with [`Puzzle::insert`], or
//!   bulk-construct with [`Puzzle::with_cages`].
//! - Enumerate solutions with [`Puzzle::solve`].
//! - Generate a random puzzle with [`Puzzle::generate`] (or [`Puzzle::generate_with`] for a custom
//!   operation policy and cage-size distribution).

#![allow(clippy::must_use_candidate, clippy::return_self_not_must_use)]

mod constraints;
mod generator;
mod grid;
mod puzzle;
mod solver;
mod types;

// Exploratory spike for issue #106 (CS-trait architecture). This branch
// (`spike/cs-trait`) carries the spike as a reference artifact for comparison
// against the parallel Pumpkin spike; it never merges to `main`.
pub mod spike;

pub(crate) use constraints::Cover;
pub use constraints::{
    cage::{
        Cage, Tuple,
        operation::{CageOption, Operation, Operator},
    },
    cage_slot::CageSlot,
    polyomino::Polyomino,
};
pub use generator::generate::SizeDistribution;
pub(crate) use grid::Grid;
pub use puzzle::Puzzle;
pub use types::{Cell, Error, Fill};
