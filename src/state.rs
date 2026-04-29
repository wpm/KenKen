use crate::puzzle::{Cage, Cell, Tuple, Value};
use sealed::DomainContent;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Display;

#[derive(Debug, PartialEq, Eq)]
pub enum StateError {
    UnknownCage,
    TupleLengthMismatch { expected: usize, got: usize },
    TupleNotFound,
    TupleAlreadyPresent,
}

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

#[allow(dead_code)]
impl State {
    fn is_valid(&self) -> bool {
        self.cell_values.values().all(|v| !v.is_empty())
    }
    fn is_solved(&self) -> bool {
        self.cell_values.values().all(Domain::is_singleton)
    }

    fn buckets_for(
        &mut self,
        cage: &Cage,
        tuple_len: usize,
    ) -> Result<&mut Vec<(Value, BTreeSet<Tuple>)>, StateError> {
        if tuple_len != cage.cells.len() {
            return Err(StateError::TupleLengthMismatch {
                expected: cage.cells.len(),
                got: tuple_len,
            });
        }
        self.cage_tuples
            .get_mut(cage)
            .ok_or(StateError::UnknownCage)
    }

    fn add_tuple(&mut self, cage: &Cage, tuple: &Tuple) -> Result<(), StateError> {
        let buckets = self.buckets_for(cage, tuple.len())?;
        let anchor = *tuple.first().ok_or(StateError::TupleLengthMismatch {
            expected: cage.cells.len(),
            got: 0,
        })?;
        match buckets.iter_mut().find(|(a, _)| *a == anchor) {
            Some((_, ts)) => {
                if !ts.insert(tuple.clone()) {
                    return Err(StateError::TupleAlreadyPresent);
                }
            }
            None => {
                buckets.push((anchor, BTreeSet::from([tuple.clone()])));
            }
        }
        for (cell, &val) in cage.cells.iter().zip(tuple.iter()) {
            if let Some(domain) = self.cell_values.get_mut(cell) {
                domain.0.insert(val);
            }
        }
        Ok(())
    }

    fn remove_tuple(&mut self, cage: &Cage, tuple: &Tuple) -> Result<(), StateError> {
        let buckets = self.buckets_for(cage, tuple.len())?;
        if !buckets.iter_mut().any(|(_, ts)| ts.remove(tuple)) {
            return Err(StateError::TupleNotFound);
        }
        let new_domains: Vec<BTreeSet<Value>> = (0..cage.cells.len())
            .map(|i| {
                buckets
                    .iter()
                    .flat_map(|(_, ts)| ts.iter())
                    .map(|t| t[i])
                    .collect()
            })
            .collect();
        for (cell, new_domain) in cage.cells.iter().zip(new_domains) {
            if let Some(domain) = self.cell_values.get_mut(cell) {
                domain.0 = new_domain;
            }
        }
        Ok(())
    }
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

#[allow(dead_code)]
impl Domain<BTreeSet<Value>> {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn is_singleton(&self) -> bool {
        self.0.len() == 1
    }
}

impl<T: DomainContent + fmt::Debug> Display for Domain<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puzzle::{BTreeSet, Operation};

    fn cell(r: usize, c: usize) -> Cell {
        (r, c)
    }

    fn domain(values: impl IntoIterator<Item = Value>) -> Domain<BTreeSet<Value>> {
        Domain(values.into_iter().collect())
    }

    fn make_cage(cells: impl IntoIterator<Item = Cell>, op: Operation) -> Cage {
        Cage {
            op,
            cells: cells.into_iter().collect(),
        }
    }

    fn solved_state() -> State {
        State {
            cell_values: BTreeMap::from([(cell(0, 0), domain([1])), (cell(0, 1), domain([2]))]),
            cage_tuples: BTreeMap::new(),
        }
    }

    fn unsolved_state() -> State {
        State {
            cell_values: BTreeMap::from([(cell(0, 0), domain([1, 2])), (cell(0, 1), domain([2]))]),
            cage_tuples: BTreeMap::new(),
        }
    }

    fn invalid_state() -> State {
        State {
            cell_values: BTreeMap::from([(cell(0, 0), domain([])), (cell(0, 1), domain([2]))]),
            cage_tuples: BTreeMap::new(),
        }
    }

    fn empty_state() -> State {
        State {
            cell_values: BTreeMap::new(),
            cage_tuples: BTreeMap::new(),
        }
    }

    #[test]
    fn is_valid_when_solved() {
        assert!(solved_state().is_valid());
    }

    #[test]
    fn is_valid_when_unsolved() {
        assert!(unsolved_state().is_valid());
    }

    #[test]
    fn is_valid_empty_domain() {
        assert!(!invalid_state().is_valid());
    }

    #[test]
    fn is_valid_empty_state() {
        assert!(empty_state().is_valid());
    }

    #[test]
    fn is_solved_all_singleton() {
        assert!(solved_state().is_solved());
    }

    #[test]
    fn is_solved_multiple_candidates() {
        assert!(!unsolved_state().is_solved());
    }

    #[test]
    fn is_solved_empty_state() {
        assert!(empty_state().is_solved());
    }

    #[test]
    fn domain_display() {
        assert_eq!(domain([1, 2, 3]).to_string(), "{1, 2, 3}");
        assert_eq!(domain([]).to_string(), "{}");
        assert_eq!(domain([5]).to_string(), "{5}");
    }

    #[test]
    fn state_display_cells_and_cages() {
        let cage = make_cage([cell(0, 0)], Operation::Given(1));
        let state = State {
            cell_values: BTreeMap::from([(cell(0, 0), domain([1]))]),
            cage_tuples: BTreeMap::from([(cage, vec![(1, BTreeSet::from([vec![1]]))])]),
        };
        let s = state.to_string();
        assert!(s.contains("Cells"));
        assert!(s.contains("(0, 0)"));
        assert!(s.contains("Cages"));
    }

    #[test]
    fn state_display_empty() {
        let state = State {
            cell_values: BTreeMap::new(),
            cage_tuples: BTreeMap::new(),
        };
        let s = state.to_string();
        assert!(s.contains("Cells"));
        assert!(s.contains("Cages"));
    }

    fn cage_state() -> (Cage, State) {
        let cage = make_cage([cell(0, 0), cell(0, 1)], Operation::Add(3));
        let state = State {
            cell_values: BTreeMap::from([(cell(0, 0), domain([])), (cell(0, 1), domain([]))]),
            cage_tuples: BTreeMap::from([(cage.clone(), vec![(1, BTreeSet::new())])]),
        };
        (cage, state)
    }

    #[test]
    fn add_tuple_updates_cage_and_cells() {
        let (cage, mut state) = cage_state();
        assert!(state.add_tuple(&cage, &vec![1, 2]).is_ok());
        let buckets = &state.cage_tuples[&cage];
        assert!(buckets.iter().any(|(_, ts)| ts.contains(&vec![1, 2])));
        assert!(state.cell_values[&cell(0, 0)].0.contains(&1));
        assert!(state.cell_values[&cell(0, 1)].0.contains(&2));
    }

    #[test]
    fn add_tuple_duplicate_returns_error() {
        let (cage, mut state) = cage_state();
        assert!(state.add_tuple(&cage, &vec![1, 2]).is_ok());
        assert_eq!(
            state.add_tuple(&cage, &vec![1, 2]),
            Err(StateError::TupleAlreadyPresent)
        );
    }

    #[test]
    fn add_tuple_length_mismatch_returns_error() {
        let (cage, mut state) = cage_state();
        assert_eq!(
            state.add_tuple(&cage, &vec![1]),
            Err(StateError::TupleLengthMismatch {
                expected: 2,
                got: 1
            })
        );
    }

    #[test]
    fn add_tuple_unknown_cage_returns_error() {
        let other_cage = make_cage([cell(1, 1)], Operation::Given(5));
        let (_, mut state) = cage_state();
        assert_eq!(
            state.add_tuple(&other_cage, &vec![5]),
            Err(StateError::UnknownCage)
        );
    }

    #[test]
    fn remove_tuple_updates_cage_and_cells() {
        let (cage, mut state) = cage_state();
        assert!(state.add_tuple(&cage, &vec![1, 2]).is_ok());
        assert!(state.add_tuple(&cage, &vec![2, 1]).is_ok());
        assert!(state.remove_tuple(&cage, &vec![1, 2]).is_ok());
        let buckets = &state.cage_tuples[&cage];
        assert!(!buckets.iter().any(|(_, ts)| ts.contains(&vec![1, 2])));
        assert!(!state.cell_values[&cell(0, 0)].0.contains(&1));
        assert!(state.cell_values[&cell(0, 0)].0.contains(&2));
    }

    #[test]
    fn remove_tuple_not_found_returns_error() {
        let (cage, mut state) = cage_state();
        assert_eq!(
            state.remove_tuple(&cage, &vec![1, 2]),
            Err(StateError::TupleNotFound)
        );
    }

    #[test]
    fn remove_tuple_length_mismatch_returns_error() {
        let (cage, mut state) = cage_state();
        assert_eq!(
            state.remove_tuple(&cage, &vec![1, 2, 3]),
            Err(StateError::TupleLengthMismatch {
                expected: 2,
                got: 3
            })
        );
    }

    #[test]
    fn remove_tuple_unknown_cage_returns_error() {
        let other_cage = make_cage([cell(1, 1)], Operation::Given(5));
        let (_, mut state) = cage_state();
        assert_eq!(
            state.remove_tuple(&other_cage, &vec![5]),
            Err(StateError::UnknownCage)
        );
    }
}
