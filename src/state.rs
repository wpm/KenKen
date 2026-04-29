use crate::puzzle::{Cage, Cell, Operation, Tuple, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fmt::Display;

#[derive(Debug, PartialEq, Eq)]
pub enum StateError {
    TupleLengthMismatch { expected: usize, got: usize },
    TupleNotFound,
}

/// Solver state for a single cage. Holds the set of candidate tuples consistent
/// with the cage's operation and the current constraints.
#[derive(Debug, Clone)]
pub struct CageState {
    cage_size: usize,
    tuples: BTreeSet<Tuple>,
}

impl CageState {
    /// Initializes a cage state with all tuples from `1..=n` that satisfy
    /// the cage's operation and assign distinct values to collinear cell pairs
    /// (cells sharing a row or column).
    #[must_use]
    pub fn new(cage: &Cage, n: usize) -> Self {
        let cells: Vec<_> = cage.cells.iter().copied().collect();
        let size = cells.len();

        // Precompute which index-pairs must have distinct values (same row or col).
        let collinear_pairs: Vec<(usize, usize)> = (0..size)
            .flat_map(|i| (i + 1..size).map(move |j| (i, j)))
            .filter(|&(i, j)| cells[i].0 == cells[j].0 || cells[i].1 == cells[j].1)
            .collect();

        let tuples = enumerate_tuples(size, n)
            .into_iter()
            .filter(|t| satisfies(&cage.op, t))
            .filter(|t| collinear_pairs.iter().all(|&(i, j)| t[i] != t[j]))
            .collect();
        Self {
            cage_size: size,
            tuples,
        }
    }

    /// Returns an iterator over all candidate tuples.
    pub fn tuples(&self) -> impl Iterator<Item = &Tuple> {
        self.tuples.iter()
    }

    /// Returns the number of distinct values each cell can take across remaining tuples.
    pub fn domain_sizes(&self, cage: &Cage) -> impl Iterator<Item = (Cell, usize)> + '_ {
        let cells: Vec<Cell> = cage.cells.iter().copied().collect();
        let size = cells.len();
        let mut seen: Vec<BTreeSet<Value>> = vec![BTreeSet::new(); size];
        for tuple in &self.tuples {
            for (i, &val) in tuple.iter().enumerate() {
                seen[i].insert(val);
            }
        }
        cells.into_iter().zip(seen.into_iter().map(|s| s.len()))
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

    /// Removes all tuples that assign `value` to `cell`, mutating the state in
    /// place. Returns the set of cells whose candidate value-sets changed, or
    /// `None` if `cell` is not in `cage`.
    pub fn remove(&mut self, value: Value, cell: Cell, cage: &Cage) -> Option<BTreeSet<Cell>> {
        let pos = cage.cells.iter().position(|&c| c == cell)?;

        let before = self.values(cage);
        self.tuples.retain(|t| t[pos] != value);
        let after = self.values(cage);

        let changed = before
            .into_iter()
            .filter(|(c, domain)| after[c] != *domain)
            .map(|(c, _)| c)
            .collect();

        Some(changed)
    }

    /// # Errors
    ///
    /// Returns `StateError::TupleLengthMismatch` if the `tuple` length differs from the cage size,
    /// or `StateError::TupleNotFound` if the tuple is not in the state.
    pub fn remove_tuple(&mut self, tuple: &[Value]) -> Result<(), StateError> {
        if tuple.len() != self.cage_size {
            return Err(StateError::TupleLengthMismatch {
                expected: self.cage_size,
                got: tuple.len(),
            });
        }
        if !self.tuples.contains(tuple as &[Value]) {
            return Err(StateError::TupleNotFound);
        }
        self.tuples.remove(tuple as &[Value]);
        Ok(())
    }
}

impl Display for CageState {
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
        let s = CageState::new(&c, 3);
        assert_eq!(s.tuples, BTreeSet::from([vec![2]]));
    }

    #[test]
    fn new_add_cage() {
        let c = cage([(0, 0), (0, 1)], Operation::Add(3));
        let s = CageState::new(&c, 3);
        assert!(s.tuples.contains(&vec![1, 2]));
        assert!(s.tuples.contains(&vec![2, 1]));
        assert!(!s.tuples.contains(&vec![3, 0]));
    }

    #[test]
    fn new_collinear_same_row_no_duplicate_values() {
        // Two cells in the same row: values must differ.
        let c = cage([(0, 0), (0, 1)], Operation::Add(4));
        let s = CageState::new(&c, 3);
        for t in &s.tuples {
            assert_ne!(t[0], t[1], "same-row cells must have distinct values");
        }
    }

    #[test]
    fn new_collinear_same_col_no_duplicate_values() {
        // Two cells in the same column: values must differ.
        let c = cage([(0, 0), (1, 0)], Operation::Add(4));
        let s = CageState::new(&c, 3);
        for t in &s.tuples {
            assert_ne!(t[0], t[1], "same-col cells must have distinct values");
        }
    }

    #[test]
    fn new_l_shaped_cage_partial_collinear_filtering() {
        // L-shaped cage: (0,0), (1,0), (1,1).
        // BTree order: (0,0) < (1,0) < (1,1), so tuple positions are 0, 1, 2.
        // Colinear pairs: (0,0)&(1,0) share col 0 → positions 0 & 1 must differ.
        //                 (1,0)&(1,1) share row 1 → positions 1 & 2 must differ.
        //                 (0,0)&(1,1) share neither → no constraint between pos 0 & 2.
        let c = cage([(0, 0), (1, 0), (1, 1)], Operation::Add(6));
        let s = CageState::new(&c, 3);
        for t in &s.tuples {
            assert_ne!(t[0], t[1], "(0,0) and (1,0) share col 0");
            assert_ne!(t[1], t[2], "(1,0) and (1,1) share row 1");
        }
    }

    #[test]
    fn new_l_shaped_mul6() {
        // L-shaped cage: (0,0) pos0, (1,0) pos1, (1,1) pos2 — 6×6 grid.
        // Collinear: pos0&pos1 (col 0), pos1&pos2 (row 1). No constraint pos0&pos2.
        // Product 6: multisets {1,1,6} → only [1,6,1] survives (pos0=pos2 ok);
        //            {1,2,3} → all 6 distinct permutations pass.
        let c = cage([(0, 0), (1, 0), (1, 1)], Operation::Mul(6));
        let s = CageState::new(&c, 6);
        assert_eq!(
            s.tuples,
            BTreeSet::from([
                vec![1, 2, 3],
                vec![1, 3, 2],
                vec![1, 6, 1],
                vec![2, 1, 3],
                vec![2, 3, 1],
                vec![3, 1, 2],
                vec![3, 2, 1],
            ])
        );
    }

    #[test]
    fn new_l_shaped_add7() {
        // L-shaped cage: (0,0) pos0, (1,0) pos1, (1,1) pos2 — 6×6 grid.
        // Collinear: pos0&pos1 (col 0), pos1&pos2 (row 1). No constraint pos0&pos2.
        // Sum 7: {1,1,5} → only [1,5,1] survives;
        //        {1,2,4} → all 6 distinct permutations pass;
        //        {1,3,3} → only [3,1,3] survives;
        //        {2,2,3} → only [2,3,2] survives.
        let c = cage([(0, 0), (1, 0), (1, 1)], Operation::Add(7));
        let s = CageState::new(&c, 6);
        assert_eq!(
            s.tuples,
            BTreeSet::from([
                vec![1, 2, 4],
                vec![1, 4, 2],
                vec![1, 5, 1],
                vec![2, 1, 4],
                vec![2, 3, 2],
                vec![2, 4, 1],
                vec![3, 1, 3],
                vec![4, 1, 2],
                vec![4, 2, 1],
            ])
        );
    }

    #[test]
    fn remove_unknown_cell_returns_none() {
        let c = cage([(0, 0), (0, 1)], Operation::Add(3));
        let mut s = CageState::new(&c, 3);
        assert!(s.remove(1, (9, 9), &c).is_none());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn remove_value_not_present_returns_unchanged() {
        // (0,0) only ever holds values ≤4 in an Add(7) 6×6 cage; removing 5 from it changes nothing.
        let c = cage([(0, 0), (1, 0), (1, 1)], Operation::Add(7));
        let tuples_before = CageState::new(&c, 6).tuples;
        let mut s = CageState::new(&c, 6);
        let changed = s.remove(5, (0, 0), &c).expect("cell in cage");
        assert_eq!(s.tuples, tuples_before);
        assert!(changed.is_empty());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn remove_affects_only_tuples_at_that_position() {
        // L-shaped +7, 6×6. Remove value 1 from cell (1,0) (pos1).
        // Tuples with t[1]==1: [2,1,4], [3,1,3], [4,1,2] are removed.
        // Remaining: [1,2,4],[1,4,2],[1,5,1],[2,3,2],[2,4,1],[4,2,1].
        //   pos0 before: {1,2,3,4}; after: {1,2,4} — 3 disappears → (0,0) changed.
        //   pos1 before: {1,2,3,4,5}; after: {2,3,4,5} — 1 disappears → (1,0) changed.
        //   pos2 before: {1,2,3,4}; after: {1,2,4} — 3 disappears → (1,1) changed.
        let c = cage([(0, 0), (1, 0), (1, 1)], Operation::Add(7));
        let mut s = CageState::new(&c, 6);
        let changed = s.remove(1, (1, 0), &c).expect("cell in cage");
        assert_eq!(
            s.tuples,
            BTreeSet::from([
                vec![1, 2, 4],
                vec![1, 4, 2],
                vec![1, 5, 1],
                vec![2, 3, 2],
                vec![2, 4, 1],
                vec![4, 2, 1],
            ])
        );
        assert_eq!(changed, BTreeSet::from([(0, 0), (1, 0), (1, 1)]));
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn remove_with_no_domain_change_at_other_cells() {
        // L-shaped +7, 6×6. Remove value 5 from cell (1,0) (pos1).
        // Only tuple with t[1]==5 is [1,5,1].
        // pos0 before {1,2,3,4}, after {1,2,3,4} — unchanged (1 still supplied by others).
        // pos1 before {1,2,3,4,5}, after {1,2,3,4} — 5 disappears → (1,0) changed.
        // pos2 before {1,2,3,4}, after {1,2,3,4} — unchanged.
        let c = cage([(0, 0), (1, 0), (1, 1)], Operation::Add(7));
        let mut s = CageState::new(&c, 6);
        let changed = s.remove(5, (1, 0), &c).expect("cell in cage");
        assert_eq!(
            s.tuples,
            BTreeSet::from([
                vec![1, 2, 4],
                vec![1, 4, 2],
                vec![2, 1, 4],
                vec![2, 3, 2],
                vec![2, 4, 1],
                vec![3, 1, 3],
                vec![4, 1, 2],
                vec![4, 2, 1],
            ])
        );
        assert_eq!(changed, BTreeSet::from([(1, 0)]));
    }

    #[test]
    fn new_sub_cage() {
        let c = cage([(0, 0), (1, 0)], Operation::Sub(1));
        let s = CageState::new(&c, 3);
        for t in &s.tuples {
            let diff = (i32::from(t[0]) - i32::from(t[1])).unsigned_abs();
            assert_eq!(diff, 1);
        }
    }

    #[test]
    fn new_mul_cage() {
        let c = cage([(0, 0), (0, 1)], Operation::Mul(6));
        let s = CageState::new(&c, 3);
        for t in &s.tuples {
            assert_eq!(u32::from(t[0]) * u32::from(t[1]), 6);
        }
    }

    #[test]
    fn new_div_cage() {
        let c = cage([(0, 0), (0, 1)], Operation::Div(2));
        let s = CageState::new(&c, 4);
        for t in &s.tuples {
            let (a, b) = (u32::from(t[0]), u32::from(t[1]));
            assert!(a * 2 == b || b * 2 == a);
        }
    }

    #[test]
    fn values_reflects_tuples() {
        let c = cage([(0, 0), (0, 1)], Operation::Add(3));
        let s = CageState::new(&c, 2);
        let vals = s.values(&c);
        assert_eq!(vals[&(0, 0)], BTreeSet::from([1, 2]));
        assert_eq!(vals[&(0, 1)], BTreeSet::from([1, 2]));
    }

    #[test]
    fn is_solved_single_tuple() {
        let c = cage([(0, 0)], Operation::Given(3));
        let s = CageState::new(&c, 3);
        assert!(s.is_solved());
    }

    #[test]
    fn is_solved_multiple_tuples() {
        let c = cage([(0, 0), (0, 1)], Operation::Add(3));
        let s = CageState::new(&c, 3);
        assert!(!s.is_solved());
    }

    #[test]
    fn is_valid_nonempty() {
        let c = cage([(0, 0)], Operation::Given(2));
        let s = CageState::new(&c, 3);
        assert!(s.is_valid());
    }

    #[test]
    fn is_valid_empty_tuples() {
        let c = cage([(0, 0)], Operation::Given(5));
        let s = CageState::new(&c, 3);
        assert!(!s.is_valid());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn remove_tuple_removes() {
        let c = cage([(0, 0)], Operation::Given(2));
        let mut s = CageState::new(&c, 3);
        s.remove_tuple(&[2]).unwrap();
        assert!(s.tuples.is_empty());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn remove_tuple_not_found_error() {
        let c = cage([(0, 0)], Operation::Given(2));
        let mut s = CageState::new(&c, 3);
        assert_eq!(s.remove_tuple(&[3]).unwrap_err(), StateError::TupleNotFound);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn remove_tuple_length_mismatch_error() {
        let c = cage([(0, 0)], Operation::Given(2));
        let mut s = CageState::new(&c, 3);
        assert_eq!(
            s.remove_tuple(&[2, 1]).unwrap_err(),
            StateError::TupleLengthMismatch {
                expected: 1,
                got: 2
            }
        );
    }
}
