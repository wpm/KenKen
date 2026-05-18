//! KenKen puzzle generator and solver.
//!
//! [`Puzzle`] is the entry point for everything the crate does:
//! - Build an empty board with [`Puzzle::new`] and add constraints with [`Puzzle::insert`], or
//!   bulk-construct with [`Puzzle::with_cages`].
//! - Enumerate solutions with [`Puzzle::solve`].
//! - Generate a random puzzle with [`Puzzle::generate`] (or [`Puzzle::generate_with`] for a custom
//!   operation policy and cage-size distribution). Requires the `generate` feature, which is on by
//!   default; disable it (e.g. with `--no-default-features`) for wasm-friendly builds that ship no
//!   `rand`/`getrandom` code.

#![allow(clippy::must_use_candidate, clippy::return_self_not_must_use)]
#![cfg_attr(not(feature = "generate"), allow(rustdoc::broken_intra_doc_links))]

mod constraints;
#[cfg(feature = "generate")]
mod generator;
mod grid;
mod puzzle;
mod solver;
mod types;

pub(crate) use constraints::Cover;
pub use constraints::{
    cage::{
        Cage, Tuple,
        operation::{Operation, Operator},
    },
    polyomino::Polyomino,
};
#[cfg(feature = "generate")]
pub use generator::generate::SizeDistribution;
pub(crate) use grid::Grid;
pub use puzzle::Puzzle;
pub use types::{Cell, Error, Fill};
