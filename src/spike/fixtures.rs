//! The one fixed test instance, shared with the parallel Pumpkin spike.
//!
//! Both spikes must solve **the same** instance and compute the same fixpoint,
//! so the comparison can't slide into "well, on a different example...". The
//! instance is a 6×6 KenKen produced by the production generator under a fixed
//! seed (reproducible, and translatable into either spike's representation).

#![allow(clippy::unwrap_used)]

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::{
    Puzzle,
    spike::{cage::CageDef, problem::PartialPuzzle},
};

/// Grid size of the shared instance.
pub const SIZE: usize = 6;

/// The fixed seed defining the shared instance. Pinning it makes the instance a
/// stable reference artifact across both spikes and across runs.
pub const SEED: u64 = 20_260_521;

/// The production-side instance: a generated 6×6 KenKen at [`SEED`]. Its cages
/// and its propagated `candidates()` are the oracle the spike is compared to.
pub fn puzzle() -> Puzzle {
    let mut rng = ChaCha8Rng::seed_from_u64(SEED);
    Puzzle::generate(SIZE, &mut rng).unwrap()
}

/// The same instance re-expressed as a spike [`PartialPuzzle`]: every cage of
/// `puzzle`, translated into a [`CageDef`] that carries no tuple state.
pub fn partial(puzzle: &Puzzle) -> PartialPuzzle {
    let mut partial = PartialPuzzle::new(puzzle.n());
    for cage in puzzle.cages() {
        partial = partial.with_cage(CageDef::new(
            cage.n(),
            cage.polyomino().clone(),
            cage.operation(),
        ));
    }
    partial
}
