use crate::M;
use strum::EnumIter;

/// An arithmetic operation that defines a polyomino.
///
/// Every variant carries its target as an `M` (u16). The product target needs the wider
/// type; the others fit in `N` but use `M`, so the API is uniform and consumers can read
/// `target` without matching on the variant.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Operation {
    Add(M),
    Subtract(M),
    Multiply(M),
    Divide(M),
    Given(M),
}

/// The operator portion of an [`Operation`], without an associated target.
///
/// Used by `Cage::valid_operators` to enumerate the operators legal for a cage shape, and by
/// `Cage::valid_targets` to select an operator for which to enumerate legal targets.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, EnumIter)]
pub enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Given,
}

#[allow(dead_code)]
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
