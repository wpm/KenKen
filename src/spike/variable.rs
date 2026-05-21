//! The CS-nomenclature [`Variable`] trait, implemented on the existing [`Cell`].

use std::hash::Hash;

use crate::{Cell, types::N};

/// Identifier for a constraint-satisfaction variable.
///
/// A thin newtype over [`Cell`]: in KenKen the variables *are* the grid cells,
/// so the identifier carries the cell coordinates directly. [`store::Store`]
/// keys its intrinsic domains by `VarId`.
///
/// [`store::Store`]: crate::spike::store::Store
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VarId(pub Cell);

/// A constraint-satisfaction variable: something with a stable identity and a
/// value type drawn from a finite domain.
pub trait Variable {
    type Value: Copy + Eq + Hash;
    fn id(&self) -> VarId;
}

impl Variable for Cell {
    type Value = N;

    fn id(&self) -> VarId {
        VarId(*self)
    }
}
