//! KenKen puzzle generator and solver.
//!
//! [`Puzzle`] is the entry point for everything the crate does:
//! - Build an empty board with [`Puzzle::new`] and add constraints with [`Puzzle::insert`], or
//!   bulk-construct with [`Puzzle::with_cages`].
//! - Enumerate solutions with [`Puzzle::solve`].
//! - Generate a random puzzle with [`Puzzle::generate`] (or [`Puzzle::generate_with`] for a custom
//!   operation policy and cage-size distribution).
//!
//! Internally, the solver is organized around standard constraint-satisfaction
//! concepts: a `Variable` trait over grid cells, a `Store` of intrinsic domains,
//! a derived viable-tuple `Cache`, `Constraint`s ([`Cage`] and `AllDifferent`)
//! propagated to a fixed point, and a depth-first search.

#![allow(clippy::must_use_candidate, clippy::return_self_not_must_use)]

mod all_different;
mod arithmetic;
mod cache;
mod cage;
mod cage_slot;
mod constraint;
mod cover;
mod generator;
mod operation;
mod polyomino;
mod puzzle;
mod solver;
mod store;
mod types;
mod variable;

#[cfg(test)]
mod test_utils;

pub use cage::{Cage, Tuple};
pub use cage_slot::CageSlot;
pub use generator::generate::{SizeDistribution, generate};
pub use operation::{CageOption, Operation, Operator};
pub use polyomino::Polyomino;
pub use puzzle::Puzzle;
pub use types::{Cell, Domain, Error};
