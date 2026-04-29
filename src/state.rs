use crate::puzzle::{Cage, Cell, Tuple, Value};
use sealed::DomainContent;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Display;

#[allow(unused)]
/// Solver state for a `KenKen` puzzle. Holds the current primal and dual
/// domains, which are refined during constraint propagation until a solution
/// is found or the search backtracks.
#[derive(Debug)]
struct State {
    /// Remaining candidate values for each cell.
    cell_values: Primal,
    /// Remaining candidate value assignments for each cage.
    cage_tuples: Dual,
}
impl Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Cells")?;
        for (cell, domain) in &self.cell_values {
            writeln!(f, "  ({cell:?}): {domain}")?;
        }
        writeln!(f, "Cages")?;
        for (cage, tuples) in &self.cage_tuples {
            writeln!(f, "  {cage}: {tuples:?}")?;
        }
        Ok(())
    }
}

/// Maps each cell to the set of values still consistent with the constraints.
type Primal = BTreeMap<Cell, Domain<BTreeSet<Value>>>;

/// Maps each cage to the candidate assignments: pairs of (anchor value, tuple
/// of remaining values) that satisfy the cage's operation.
type Dual = BTreeMap<Cage, Vec<(Value, BTreeSet<Tuple>)>>;

/// Seals `DomainContent` so only `BTreeSet<Value>` and `BTreeSet<Tuple>` can
/// be used as domain contents.
pub mod sealed {
    use crate::puzzle::{BTreeSet, Tuple, Value};
    pub trait DomainContent {}
    impl DomainContent for BTreeSet<Value> {}
    impl DomainContent for BTreeSet<Tuple> {}
}

/// A typed domain: either a set of candidate values (primal) or a set of
/// candidate tuples (dual). The type parameter is sealed to those two cases.
#[derive(Debug)]
struct Domain<T: DomainContent>(T);

impl<T: DomainContent + fmt::Debug> Display for Domain<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}
