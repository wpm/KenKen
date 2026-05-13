use crate::types::M;
use strum::EnumIter;

/// An arithmetic operation that defines a polyomino.
///
/// Every variant carries its target as an `M` (u16). The product target needs the wider
/// type; the others fit in `N` but use `M`, so the API is uniform and consumers can read
/// `target` without matching on the variant.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Operation {
    /// Cells sum to the target.
    Add(M),
    /// Two cells differ by the target.
    Subtract(M),
    /// Cells multiply to the target.
    Multiply(M),
    /// Two cells have a ratio equal to the target.
    Divide(M),
    /// A single cell is fixed to the target value.
    Given(M),
}

/// The operator portion of an [`Operation`], without an associated target.
///
/// Used by `Cage::valid_operators` to enumerate the operators legal for a cage shape, and by
/// `Cage::valid_targets` to select an operator for which to enumerate legal targets.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, EnumIter)]
pub enum Operator {
    /// Cells sum to a target.
    Add,
    /// Two cells differ by a target.
    Subtract,
    /// Cells multiply to a target.
    Multiply,
    /// Two cells have a ratio equal to a target.
    Divide,
    /// A single cell is fixed to a target value.
    Given,
}

impl Operator {
    /// Returns the operator of `operation`.
    #[must_use]
    pub const fn of(operation: Operation) -> Self {
        match operation {
            Operation::Add(_) => Self::Add,
            Operation::Subtract(_) => Self::Subtract,
            Operation::Multiply(_) => Self::Multiply,
            Operation::Divide(_) => Self::Divide,
            Operation::Given(_) => Self::Given,
        }
    }
}
