use crate::puzzle::{Cage, Cell, Operation, Tuple, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Display;

#[derive(Debug, PartialEq, Eq)]
pub enum StateError {
    TupleLengthMismatch { expected: usize, got: usize },
    TupleNotFound,
    TupleAlreadyPresent,
}

/// Solver state for a single cage. Holds the set of candidate tuples consistent
/// with the cage's operation and the current constraints.
#[derive(Debug, Clone)]
pub struct State {
    cage_size: usize,
    tuples: BTreeSet<Tuple>,
}

impl State {
    /// Initializes a cage state with all tuples from `1..=n` that satisfy
    /// the cage's operation.
    #[must_use]
    pub fn new(cage: &Cage, n: usize) -> Self {
        let size = cage.cells.len();
        let tuples = enumerate_tuples(size, n)
            .into_iter()
            .filter(|t| satisfies(&cage.op, t))
            .collect();
        Self {
            cage_size: size,
            tuples,
        }
    }

    /// Returns a map from each cell in the cage to the set of values it can
    /// take across all remaining tuples, in cage-cell order.
    #[must_use]
    pub fn values(&self, cage: &Cage) -> BTreeMap<Cell, BTreeSet<Value>> {
        let mut map: BTreeMap<Cell, BTreeSet<Value>> =
            cage.cells.iter().map(|&c| (c, BTreeSet::new())).collect();
        for tuple in &self.tuples {
            for (&cell, &val) in cage.cells.iter().zip(tuple.iter()) {
                if let Some(s) = map.get_mut(&cell) {
                    s.insert(val);
                }
            }
        }
        map
    }

    /// Returns true if every cell has exactly one candidate value.
    #[must_use]
    pub fn is_solved(&self) -> bool {
        self.tuples.len() == 1
    }

    /// Returns true if every cell has at least one candidate value.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.tuples.is_empty()
    }

    /// # Errors
    ///
    /// Returns `StateError::TupleLengthMismatch` if `tuple` length differs from the cage size,
    /// or `StateError::TupleAlreadyPresent` if the tuple is already in the state.
    pub fn add_tuple(&self, tuple: &[Value]) -> Result<Self, StateError> {
        if tuple.len() != self.cage_size {
            return Err(StateError::TupleLengthMismatch {
                expected: self.cage_size,
                got: tuple.len(),
            });
        }
        if self.tuples.contains(tuple as &[Value]) {
            return Err(StateError::TupleAlreadyPresent);
        }
        let mut tuples = self.tuples.clone();
        tuples.insert(tuple.to_vec());
        Ok(Self {
            cage_size: self.cage_size,
            tuples,
        })
    }

    /// # Errors
    ///
    /// Returns `StateError::TupleLengthMismatch` if `tuple` length differs from the cage size,
    /// or `StateError::TupleNotFound` if the tuple is not in the state.
    pub fn remove_tuple(&self, tuple: &[Value]) -> Result<Self, StateError> {
        if tuple.len() != self.cage_size {
            return Err(StateError::TupleLengthMismatch {
                expected: self.cage_size,
                got: tuple.len(),
            });
        }
        if !self.tuples.contains(tuple as &[Value]) {
            return Err(StateError::TupleNotFound);
        }
        let mut tuples = self.tuples.clone();
        tuples.remove(tuple as &[Value]);
        Ok(Self {
            cage_size: self.cage_size,
            tuples,
        })
    }
}

impl Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner: Vec<String> = self.tuples.iter().map(|t| format!("{t:?}")).collect();
        write!(f, "{{{}}}", inner.join(", "))
    }
}

/// Enumerates all tuples of length `size` with values in `1..=n`.
/// Tuple positions correspond to cells in BTree-sorted order.
fn enumerate_tuples(size: usize, n: usize) -> Vec<Tuple> {
    if size == 0 {
        return vec![vec![]];
    }
    let sub = enumerate_tuples(size - 1, n);
    let mut result = Vec::with_capacity(sub.len() * n);
    for rest in sub {
        #[allow(clippy::cast_possible_truncation)]
        for v in 1..=(n as Value) {
            let mut t = rest.clone();
            t.push(v);
            result.push(t);
        }
    }
    result
}

/// Returns true if `tuple` satisfies `op`.
fn satisfies(op: &Operation, tuple: &Tuple) -> bool {
    match op {
        Operation::Given(v) => tuple.len() == 1 && tuple[0] == *v,
        Operation::Add(target) => tuple.iter().map(|&x| u32::from(x)).sum::<u32>() == *target,
        Operation::Mul(target) => tuple.iter().map(|&x| u32::from(x)).product::<u32>() == *target,
        Operation::Sub(target) => {
            if tuple.len() != 2 {
                return false;
            }
            let (a, b) = (u32::from(tuple[0]), u32::from(tuple[1]));
            a.abs_diff(b) == *target
        }
        Operation::Div(target) => {
            if tuple.len() != 2 {
                return false;
            }
            let (a, b) = (u32::from(tuple[0]), u32::from(tuple[1]));
            a * *target == b || b * *target == a
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puzzle::Operation;

    fn cage(cells: impl IntoIterator<Item = Cell>, op: Operation) -> Cage {
        Cage {
            op,
            cells: cells.into_iter().collect(),
        }
    }

    #[test]
    fn new_given_cage() {
        let c = cage([(0, 0)], Operation::Given(2));
        let s = State::new(&c, 3);
        assert_eq!(s.tuples, BTreeSet::from([vec![2]]));
    }

    #[test]
    fn new_add_cage() {
        let c = cage([(0, 0), (0, 1)], Operation::Add(3));
        let s = State::new(&c, 3);
        assert!(s.tuples.contains(&vec![1, 2]));
        assert!(s.tuples.contains(&vec![2, 1]));
        assert!(!s.tuples.contains(&vec![3, 0]));
    }

    #[test]
    fn new_sub_cage() {
        let c = cage([(0, 0), (1, 0)], Operation::Sub(1));
        let s = State::new(&c, 3);
        for t in &s.tuples {
            let diff = (i32::from(t[0]) - i32::from(t[1])).unsigned_abs();
            assert_eq!(diff, 1);
        }
    }

    #[test]
    fn new_mul_cage() {
        let c = cage([(0, 0), (0, 1)], Operation::Mul(6));
        let s = State::new(&c, 3);
        for t in &s.tuples {
            assert_eq!(u32::from(t[0]) * u32::from(t[1]), 6);
        }
    }

    #[test]
    fn new_div_cage() {
        let c = cage([(0, 0), (0, 1)], Operation::Div(2));
        let s = State::new(&c, 4);
        for t in &s.tuples {
            let (a, b) = (u32::from(t[0]), u32::from(t[1]));
            assert!(a * 2 == b || b * 2 == a);
        }
    }

    #[test]
    fn values_reflects_tuples() {
        let c = cage([(0, 0), (0, 1)], Operation::Add(3));
        let s = State::new(&c, 2);
        let vals = s.values(&c);
        assert_eq!(vals[&(0, 0)], BTreeSet::from([1, 2]));
        assert_eq!(vals[&(0, 1)], BTreeSet::from([1, 2]));
    }

    #[test]
    fn is_solved_single_tuple() {
        let c = cage([(0, 0)], Operation::Given(3));
        let s = State::new(&c, 3);
        assert!(s.is_solved());
    }

    #[test]
    fn is_solved_multiple_tuples() {
        let c = cage([(0, 0), (0, 1)], Operation::Add(3));
        let s = State::new(&c, 3);
        assert!(!s.is_solved());
    }

    #[test]
    fn is_valid_nonempty() {
        let c = cage([(0, 0)], Operation::Given(2));
        let s = State::new(&c, 3);
        assert!(s.is_valid());
    }

    #[test]
    fn is_valid_empty_tuples() {
        let c = cage([(0, 0)], Operation::Given(5));
        let s = State::new(&c, 3);
        assert!(!s.is_valid());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn add_tuple_inserts() {
        let s = State {
            cage_size: 2,
            tuples: BTreeSet::new(),
        };
        let s2 = s.add_tuple(&[2, 3]).unwrap();
        assert!(s2.tuples.contains(&vec![2, 3]));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn add_tuple_duplicate_error() {
        let c = cage([(0, 0)], Operation::Given(1));
        let s = State::new(&c, 3);
        assert_eq!(
            s.add_tuple(&[1]).unwrap_err(),
            StateError::TupleAlreadyPresent
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn add_tuple_length_mismatch_error() {
        let c = cage([(0, 0), (0, 1)], Operation::Add(3));
        let s = State::new(&c, 3);
        assert_eq!(
            s.add_tuple(&[1]).unwrap_err(),
            StateError::TupleLengthMismatch {
                expected: 2,
                got: 1
            }
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn remove_tuple_removes() {
        let c = cage([(0, 0)], Operation::Given(2));
        let s = State::new(&c, 3);
        let s2 = s.remove_tuple(&[2]).unwrap();
        assert!(s2.tuples.is_empty());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn remove_tuple_not_found_error() {
        let c = cage([(0, 0)], Operation::Given(2));
        let s = State::new(&c, 3);
        assert_eq!(s.remove_tuple(&[3]).unwrap_err(), StateError::TupleNotFound);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn remove_tuple_length_mismatch_error() {
        let c = cage([(0, 0)], Operation::Given(2));
        let s = State::new(&c, 3);
        assert_eq!(
            s.remove_tuple(&[2, 1]).unwrap_err(),
            StateError::TupleLengthMismatch {
                expected: 1,
                got: 2
            }
        );
    }
}
