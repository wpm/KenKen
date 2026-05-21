//! A CSP engine backed by [`pumpkin-solver`](https://docs.rs/pumpkin-solver/0.3.0).
//!
//! [`Engine`] wraps a Pumpkin `Solver` behind a small internal surface keyed on
//! the crate's domain types ([`Cell`](crate::Cell), [`Fill`](crate::Fill)).
//! Pumpkin is an implementation detail and is never re-exported.
//!
//! This module is built incrementally over the migration described in #98:
//! per-cell decision variables, row/column all-different, and a custom
//! [`cage_propagator`] for each cage. The Régin and observer propagators (and
//! the eventual removal of the bespoke solver) arrive in later steps, at which
//! point these entry points become load-bearing for `Puzzle`.
//!
//! Until the bespoke solver is retired (#98 step 5), nothing in the crate's
//! product path consumes the engine, so its surface is allowed to sit unused.
#![allow(dead_code, unused_imports)]

mod cage_propagator;
mod observer_propagator;
mod regin_propagator;
mod wrapper;

pub use observer_propagator::DomainMap;
pub use wrapper::{Engine, Solution};

use crate::types::N;

/// Narrows a solved Pumpkin value to a cell value. Engine domains are built
/// from grid values in `1..=9`, so the conversion is always in range.
pub fn value_of(value: i32) -> N {
    N::try_from(value).unwrap_or_else(|_| unreachable!("engine domain values lie in 1..=9"))
}
