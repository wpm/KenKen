//! A CSP engine backed by [`pumpkin-solver`](https://docs.rs/pumpkin-solver/0.3.0).
//!
//! [`Engine`] wraps a Pumpkin `Solver` behind a small internal surface keyed on
//! the crate's domain types ([`Cell`](crate::Cell), [`Fill`](crate::Fill)).
//! Pumpkin is an implementation detail and is never re-exported.
//!
//! This module is built incrementally over the migration described in #98:
//! step 1 registers one decision variable per cell and runs a constraint-free
//! solve to confirm the puzzle round-trips through Pumpkin. The cage, Régin,
//! and observer propagators (and the eventual removal of the bespoke solver)
//! arrive in later steps, at which point these entry points become load-bearing
//! for `Puzzle`.
//!
//! Until the bespoke solver is retired (#98 step 5), nothing in the crate's
//! product path consumes the engine, so its surface is allowed to sit unused.
#![allow(dead_code, unused_imports)]

mod wrapper;

pub use wrapper::{Engine, Solution};
