//! A CSP engine backed by [`pumpkin-solver`](https://docs.rs/pumpkin-solver/0.3.0).
//!
//! [`Engine`] wraps a Pumpkin `Solver` behind a small internal surface keyed on
//! the crate's domain types ([`Cell`](crate::Cell), [`Fill`](crate::Fill)).
//! Pumpkin is an implementation detail and is never re-exported.
//!
//! The engine models a puzzle with one decision variable per cell, a custom
//! [`regin_propagator`] enforcing GAC all-different on each row and column, a
//! custom [`cage_propagator`] per cage, and an [`observer_propagator`] that
//! reads back fixpoint domains. [`Puzzle`](crate::Puzzle) routes its
//! propagation (construction) and search ([`solve`](crate::Puzzle::solve))
//! through it; the bespoke solver has been removed (#98 step 5).
//!
//! A few entry points — `Engine::enumerate` and the `DomainMap` re-export — are
//! part of the Designer-facing surface and are not yet consumed within this
//! crate, so the module tolerates an unused surface.
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
