use std::collections::{BTreeSet, HashMap};

use crate::{
    Cell, Error, Operation, Operator,
    constraints::{
        Cover,
        cage::{
            Tuple,
            arithmetic::{
                addition_multisets, division_multisets, multiplication_multisets,
                subtraction_multisets,
            },
        },
    },
    types::{Index, M, N},
};

/// A contiguous region of edge-connected [`Cell`]s.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Polyomino(BTreeSet<Cell>);

impl Polyomino {
    /// Constructs a polyomino from a slice of cells.
    ///
    /// # Errors
    /// Returns [`Error::EmptyPolyomino`] if `cells` is empty, or
    /// [`Error::DisconnectedPolyomino`] if `cells` is not edge-connected.
    pub fn from_cells(cells: &[Cell]) -> Result<Self, Error> {
        Self::new(cells.iter().copied().collect())
    }

    /// Constructs a polyomino from a set of cells, storing them in sorted order.
    ///
    /// # Errors
    /// Returns [`Error::EmptyPolyomino`] if `cells` is empty, or
    /// [`Error::DisconnectedPolyomino`] if `cells` is not edge-connected.
    fn new(cells: BTreeSet<Cell>) -> Result<Self, Error> {
        if cells.is_empty() {
            return Err(Error::EmptyPolyomino);
        }
        if !is_edge_connected_component(&cells) {
            return Err(Error::DisconnectedPolyomino);
        }
        Ok(Self(cells))
    }

    /// Returns `true` if this polyomino and `other` share at least one cell.
    pub fn intersects(&self, other: &Self) -> bool {
        self.0.intersection(&other.0).next().is_some()
    }

    /// Returns a new polyomino with `cell` added.
    ///
    /// Idempotent: if `cell` is already present, returns an equivalent
    /// polyomino.
    ///
    /// # Errors
    /// Returns [`Error::DisconnectedPolyomino`] if adding `cell` would make the
    /// polyomino disconnected.
    #[allow(unused_results)]
    pub fn insert(&self, cell: Cell) -> Result<Self, Error> {
        let mut cells = self.0.clone();
        cells.insert(cell);
        Self::new(cells)
    }

    /// Returns a new polyomino with `cell` removed.
    ///
    /// Idempotent: if `cell` is not present, returns an equivalent polyomino.
    ///
    /// # Errors
    /// Returns [`Error::EmptyPolyomino`] if removing `cell` would empty the
    /// polyomino, or [`Error::DisconnectedPolyomino`] if it would leave the
    /// remaining cells disconnected.
    pub fn remove(&self, cell: Cell) -> Result<Self, Error> {
        let cells = self
            .0
            .iter()
            .copied()
            .filter(|c| *c != cell)
            .collect::<Vec<_>>();
        Self::from_cells(&cells)
    }

    /// Returns the operators legal for a cage covering `cells`.
    ///
    /// Singleton cages permit only [`Operator::Given`]; 2-cell cages permit all
    /// operators; larger cages permit only [`Operator::Add`] and
    /// [`Operator::Multiply`].
    pub fn valid_operators(cells: &[Cell]) -> Vec<Operator> {
        match cells.len() {
            0 => vec![],
            1 => vec![Operator::Given],
            2 => vec![
                Operator::Add,
                Operator::Subtract,
                Operator::Multiply,
                Operator::Divide,
            ],
            _ => vec![Operator::Add, Operator::Multiply],
        }
    }

    /// Returns an iterator over [`Operation`] values whose `(operator, target)`
    /// pair is legal for a cage covering `cells` on an `n`×`n` grid, in
    /// ascending target order.
    ///
    /// If `op` is not in [`Self::valid_operators`] for `cells`, the iterator is
    /// empty.
    ///
    /// # Errors
    /// Returns [`Error::EmptyPolyomino`] or [`Error::DisconnectedPolyomino`] if
    /// `cells` do not form a valid polyomino.
    pub fn valid_operations(
        cells: &[Cell],
        op: Operator,
        n: N,
    ) -> Result<Box<dyn Iterator<Item = Operation>>, Error> {
        if !Self::valid_operators(cells).contains(&op) {
            return Ok(Box::new(std::iter::empty()));
        }
        let p = Self::from_cells(cells)?;
        let k = p.len();
        let n_m = M::from(n);
        Ok(match op {
            Operator::Given => Box::new((1..=n_m).map(Operation::Given)),
            Operator::Subtract => Box::new((1..=n_m.saturating_sub(1)).map(Operation::Subtract)),
            Operator::Divide => Box::new((2..=n_m).map(Operation::Divide)),
            Operator::Add => {
                let max = M::try_from(usize::from(n).saturating_mul(k)).unwrap_or(M::MAX);
                Box::new((1..=max).filter_map(move |t| {
                    let op = Operation::Add(t);
                    p.has_admissible_tuple(n, op).then_some(op)
                }))
            }
            Operator::Multiply => {
                let exp = u32::try_from(k).unwrap_or(u32::MAX);
                let max = n_m.saturating_pow(exp);
                Box::new((1..=max).filter_map(move |t| {
                    let op = Operation::Multiply(t);
                    p.has_admissible_tuple(n, op).then_some(op)
                }))
            }
        })
    }

    /// Returns true if `operation` is legal for a cage of the given cells on an
    /// `n`×`n` grid.
    ///
    /// The operator must be in [`Self::valid_operators`] for the cell count,
    /// the target must be in its conventional range, and at least one tuple
    /// must survive collinearity filtering.
    ///
    /// # Errors
    /// Returns [`Error::EmptyPolyomino`] or [`Error::DisconnectedPolyomino`] if
    /// `cells` do not form a valid polyomino.
    pub fn is_valid_operation(cells: &[Cell], operation: Operation, n: N) -> Result<bool, Error> {
        if !Self::valid_operators(cells).contains(&Operator::of(operation)) {
            return Ok(false);
        }
        match operation {
            Operation::Given(v) => return Ok((1..=M::from(n)).contains(&v)),
            Operation::Subtract(0) | Operation::Divide(0 | 1) => return Ok(false),
            _ => {}
        }
        let p = Self::from_cells(cells)?;
        Ok(p.has_admissible_tuple(n, operation))
    }

    // TODO Make valid_tuples return an iterator so that has_admissible_tuple can exit early.
    fn has_admissible_tuple(&self, n: N, operation: Operation) -> bool {
        !self.valid_tuples(n, operation).is_empty()
    }

    /// Returns the valid ordered tuples for a single known `operation` on this
    /// polyomino on an `n`×`n` grid.
    pub(crate) fn valid_tuples(&self, n: N, operation: Operation) -> Vec<Tuple> {
        let k = self.len();
        let pairs = self.collinear_pairs();
        match operation {
            Operation::Given(v) => N::try_from(v)
                .map_or_else(|_| vec![], |v_n| ordered_tuples(&[v_n], &pairs).collect()),
            Operation::Subtract(d) => N::try_from(d).map_or_else(
                |_| vec![],
                |d_n| {
                    subtraction_multisets(n, d_n)
                        .flat_map(|ms| ordered_tuples(&ms, &pairs).collect::<Vec<_>>())
                        .collect()
                },
            ),
            Operation::Divide(q) => N::try_from(q).map_or_else(
                |_| vec![],
                |q_n| {
                    division_multisets(n, q_n)
                        .flat_map(|ms| ordered_tuples(&ms, &pairs).collect::<Vec<_>>())
                        .collect()
                },
            ),
            Operation::Add(s) => N::try_from(s).map_or_else(
                |_| vec![],
                |s_n| {
                    addition_multisets(n, k, s_n)
                        .flat_map(|ms| ordered_tuples(&ms, &pairs).collect::<Vec<_>>())
                        .collect()
                },
            ),
            Operation::Multiply(s) => multiplication_multisets(n, k, s)
                .flat_map(|ms| ordered_tuples(&ms, &pairs).collect::<Vec<_>>())
                .collect(),
        }
    }

    /// Returns pairs of cell indices that share a row or column.
    ///
    /// Each pair `(i, j)` with `i < j` means `cells[i]` and `cells[j]` must
    /// hold distinct values.
    pub fn collinear_pairs(&self) -> Vec<(usize, usize)> {
        let mut by_row: HashMap<Index, Vec<usize>> = HashMap::new();
        let mut by_col: HashMap<Index, Vec<usize>> = HashMap::new();
        for (i, cell) in self.cells().enumerate() {
            by_row.entry(cell.row).or_default().push(i);
            by_col.entry(cell.column).or_default().push(i);
        }
        let mut pairs = Vec::new();
        for group in by_row.into_values().chain(by_col.into_values()) {
            for a in 0..group.len() {
                for b in (a + 1)..group.len() {
                    pairs.push((group[a], group[b]));
                }
            }
        }
        pairs
    }

    /// Returns a map from each valid [`Operation`] to the ordered tuples that
    /// realize it, for the given operator applied to this polyomino on an
    /// `n`×`n` grid.
    ///
    /// Each key is an `(operator, target)` pair for which at least one
    /// assignment of grid values to the polyomino's cells satisfies the
    /// operation and the collinearity constraints.
    ///
    /// Subtract and Divide are only valid for 2-cell polyominoes; any other
    /// size yields an empty map.
    pub fn operator_tuples(&self, n: N, operator: Operator) -> HashMap<Operation, Vec<Tuple>> {
        let k = self.len();
        let pairs = self.collinear_pairs();

        match operator {
            Operator::Given => {
                if k != 1 {
                    return HashMap::new();
                }
                (1..=n)
                    .map(|v| (Operation::Given(M::from(v)), vec![vec![v]]))
                    .collect()
            }
            Operator::Subtract => {
                if k != 2 {
                    return HashMap::new();
                }
                (1..n)
                    .flat_map(|d| {
                        let op = Operation::Subtract(M::from(d));
                        subtraction_multisets(n, d)
                            .flat_map(|ms| {
                                ordered_tuples(&ms, &pairs)
                                    .map(move |t| (op, t))
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>()
                    })
                    .fold(HashMap::new(), |mut map, (op, t)| {
                        map.entry(op).or_default().push(t);
                        map
                    })
            }
            Operator::Divide => {
                if k != 2 {
                    return HashMap::new();
                }
                (2..=n)
                    .flat_map(|q| {
                        let op = Operation::Divide(M::from(q));
                        division_multisets(n, q)
                            .flat_map(|ms| {
                                ordered_tuples(&ms, &pairs)
                                    .map(move |t| (op, t))
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>()
                    })
                    .fold(HashMap::new(), |mut map, (op, t)| {
                        map.entry(op).or_default().push(t);
                        map
                    })
            }
            Operator::Add => {
                if k < 2 {
                    return HashMap::new();
                }
                #[allow(clippy::cast_possible_truncation)]
                let max_target = M::from(n) * M::try_from(k).unwrap_or(M::MAX);
                (1..=max_target)
                    .filter_map(|s| {
                        N::try_from(s).map_or(None, |s_n| {
                            let tuples: Vec<Tuple> = addition_multisets(n, k, s_n)
                                .flat_map(|ms| ordered_tuples(&ms, &pairs).collect::<Vec<_>>())
                                .collect();
                            if tuples.is_empty() {
                                None
                            } else {
                                Some((Operation::Add(s), tuples))
                            }
                        })
                    })
                    .collect()
            }
            Operator::Multiply => {
                if k < 2 {
                    return HashMap::new();
                }
                #[allow(clippy::cast_possible_truncation)]
                let max_target = M::from(n).saturating_pow(u32::try_from(k).unwrap_or(u32::MAX));
                (1..=max_target)
                    .filter_map(|s| {
                        let tuples: Vec<Tuple> = multiplication_multisets(n, k, s)
                            .flat_map(|ms| ordered_tuples(&ms, &pairs).collect::<Vec<_>>())
                            .collect();
                        if tuples.is_empty() {
                            None
                        } else {
                            Some((Operation::Multiply(s), tuples))
                        }
                    })
                    .collect()
            }
        }
    }
}

impl Cover for Polyomino {
    fn cells(&self) -> impl Iterator<Item = Cell> {
        self.0.iter().copied()
    }
}

/// Returns an iterator over all ordered permutations of `multiset` that satisfy
/// the collinearity constraint: for every `(i, j)` in `pairs`, positions `i`
/// and `j` must differ.
pub fn ordered_tuples<'a>(
    multiset: &'a [N],
    pairs: &'a [(usize, usize)],
) -> impl Iterator<Item = Tuple> + 'a {
    permutations(multiset).filter(move |t| pairs.iter().all(|&(i, j)| t[i] != t[j]))
}

/// Returns an iterator over all distinct permutations of `values` in
/// lexicographic order.
pub fn permutations(values: &[N]) -> impl Iterator<Item = Tuple> {
    let mut perm = values.to_vec();
    perm.sort_unstable();
    let mut all = vec![perm.clone()];
    while next_permutation(&mut perm) {
        all.push(perm.clone());
    }
    all.into_iter()
}

/// Advances `perm` to the next lexicographic permutation in place. Returns
/// `false` if it was already the last permutation.
fn next_permutation(perm: &mut [N]) -> bool {
    let n = perm.len();
    if n < 2 {
        return false;
    }
    let mut i = n - 1;
    while i > 0 && perm[i - 1] >= perm[i] {
        i -= 1;
    }
    if i == 0 {
        return false;
    }
    let pivot = i - 1;
    let mut j = n - 1;
    while perm[j] <= perm[pivot] {
        j -= 1;
    }
    perm.swap(pivot, j);
    perm[i..].reverse();
    true
}

/// Do `cells` form a contiguous edge-connected component?
/// Two `cell`s are edge-connected if they share a common edge.
pub fn is_edge_connected_component(cells: &BTreeSet<Cell>) -> bool {
    let Some(&start) = cells.first() else {
        return true;
    };
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![start];
    while let Some(cell) = stack.pop() {
        if visited.insert(cell) {
            for neighbor in cell.neighbors_4() {
                if cells.contains(&neighbor) {
                    stack.push(neighbor);
                }
            }
        }
    }
    visited.len() == cells.len()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::constraints::{
        Cover,
        test_utils::{c00, c01, c02, c10, c11, cells, col_pair, l_shape, pair, row3, singleton},
    };

    fn btree(positions: &[(usize, usize)]) -> BTreeSet<Cell> {
        cells(positions).into_iter().collect()
    }

    fn singleton_cells() -> Vec<Cell> {
        cells(&[(0, 0)])
    }
    fn pair_cells() -> Vec<Cell> {
        cells(&[(0, 0), (0, 1)])
    }
    fn row3_cells() -> Vec<Cell> {
        cells(&[(0, 0), (0, 1), (0, 2)])
    }
    fn l_shape_cells() -> Vec<Cell> {
        cells(&[(0, 0), (0, 1), (1, 0)])
    }
    fn disconnected_cells() -> Vec<Cell> {
        cells(&[(0, 0), (1, 1)])
    }

    // --- is_edge_adjacent ---

    #[test]
    fn is_edge_adjacent_empty_is_true() {
        assert!(is_edge_connected_component(&BTreeSet::new()));
    }

    #[test]
    fn is_edge_adjacent_single_cell_is_true() {
        assert!(is_edge_connected_component(&btree(&[(0, 0)])));
    }

    #[test]
    fn is_edge_adjacent_horizontal_pair_is_true() {
        assert!(is_edge_connected_component(&btree(&[(0, 0), (0, 1)])));
    }

    #[test]
    fn is_edge_adjacent_vertical_pair_is_true() {
        assert!(is_edge_connected_component(&btree(&[(0, 0), (1, 0)])));
    }

    #[test]
    fn is_edge_adjacent_diagonal_pair_is_false() {
        assert!(!is_edge_connected_component(&btree(&[(0, 0), (1, 1)])));
    }

    #[test]
    fn is_edge_adjacent_l_shape_is_true() {
        assert!(is_edge_connected_component(&btree(&[
            (0, 0),
            (1, 0),
            (1, 1)
        ])));
    }

    #[test]
    fn is_edge_adjacent_disconnected_is_false() {
        assert!(!is_edge_connected_component(&btree(&[(0, 0), (0, 2)])));
    }

    // --- Polyomino ---

    #[test]
    fn polyomino_cells_are_sorted() {
        let r = Polyomino::from_cells(&[c10(), c00(), c01()]).unwrap();
        itertools::assert_equal(r.cells(), [c00(), c01(), c10()]);
    }

    #[test]
    fn polyomino_len_matches_cell_count() {
        assert_eq!(row3().len(), 3);
    }

    #[test]
    fn polyomino_new_empty_returns_err() {
        assert!(matches!(
            Polyomino::from_cells(&[]),
            Err(Error::EmptyPolyomino)
        ));
    }

    #[test]
    fn polyomino_new_disconnected_returns_err() {
        assert!(matches!(
            Polyomino::from_cells(&[c00(), c02()]),
            Err(Error::DisconnectedPolyomino)
        ));
    }

    #[test]
    fn polyomino_new_distinguishes_empty_from_disconnected() {
        // #45: empty and disconnected inputs must report distinct errors.
        assert!(matches!(
            Polyomino::from_cells(&[]),
            Err(Error::EmptyPolyomino)
        ));
        assert!(matches!(
            Polyomino::from_cells(&[c00(), c11()]),
            Err(Error::DisconnectedPolyomino)
        ));
    }

    #[test]
    fn polyomino_new_rejects_diagonal_only_inputs() {
        // #46: a diagonal-only set of cells is not edge-connected and must be rejected.
        assert!(matches!(
            Polyomino::from_cells(&[c00(), c11(), Cell::new(2, 2)]),
            Err(Error::DisconnectedPolyomino)
        ));
    }

    // --- permutations / next_permutation ---

    #[test]
    fn permutations_single_element() {
        itertools::assert_equal(permutations(&[1]), [vec![1]]);
    }

    #[test]
    fn permutations_two_distinct_elements() {
        itertools::assert_equal(permutations(&[1, 2]), [vec![1, 2], vec![2, 1]]);
    }

    #[test]
    fn permutations_two_equal_elements() {
        itertools::assert_equal(permutations(&[2, 2]), [vec![2, 2]]);
    }

    #[test]
    fn permutations_three_distinct_elements_count() {
        assert_eq!(permutations(&[1, 2, 3]).count(), 6);
    }

    #[test]
    fn permutations_multiset_with_repeat() {
        // [1,1,2] has 3 distinct permutations
        assert_eq!(permutations(&[1, 1, 2]).count(), 3);
    }

    // --- ordered_tuples ---

    #[test]
    fn ordered_tuples_no_pairs_returns_all_permutations() {
        let result: Vec<Tuple> = ordered_tuples(&[1, 2], &[]).collect();
        assert_eq!(result, vec![vec![1, 2], vec![2, 1]]);
    }

    #[test]
    fn ordered_tuples_same_row_filters_equal_values() {
        assert!(ordered_tuples(&[2, 2], &[(0, 1)]).next().is_none());
    }

    // --- valid Polyomino operators ---

    #[test]
    fn cage_valid_operators_singleton() {
        assert_eq!(
            Polyomino::valid_operators(&singleton_cells()),
            vec![Operator::Given]
        );
    }

    #[test]
    fn cage_valid_operators_two_cells() {
        assert_eq!(
            Polyomino::valid_operators(&pair_cells()),
            vec![
                Operator::Add,
                Operator::Subtract,
                Operator::Multiply,
                Operator::Divide
            ]
        );
    }

    #[test]
    fn cage_valid_operators_three_cells() {
        assert_eq!(
            Polyomino::valid_operators(&row3_cells()),
            vec![Operator::Add, Operator::Multiply]
        );
    }

    #[test]
    fn valid_operators_empty_cells_is_empty() {
        assert!(Polyomino::valid_operators(&[]).is_empty());
    }

    #[test]
    fn cage_is_valid_singleton_given_in_range() {
        let cs = singleton_cells();
        assert!(Polyomino::is_valid_operation(&cs, Operation::Given(1), 5).unwrap());
        assert!(Polyomino::is_valid_operation(&cs, Operation::Given(5), 5).unwrap());
        assert!(!Polyomino::is_valid_operation(&cs, Operation::Given(0), 5).unwrap());
        assert!(!Polyomino::is_valid_operation(&cs, Operation::Given(6), 5).unwrap());
    }

    #[test]
    fn cage_is_valid_two_cell_subtract_zero_rejected() {
        let cs = pair_cells();
        assert!(!Polyomino::is_valid_operation(&cs, Operation::Subtract(0), 5).unwrap());
        assert!(Polyomino::is_valid_operation(&cs, Operation::Subtract(1), 5).unwrap());
    }

    #[test]
    fn cage_is_valid_two_cell_divide_below_two_rejected() {
        let cs = pair_cells();
        assert!(!Polyomino::is_valid_operation(&cs, Operation::Divide(0), 5).unwrap());
        assert!(!Polyomino::is_valid_operation(&cs, Operation::Divide(1), 5).unwrap());
        assert!(Polyomino::is_valid_operation(&cs, Operation::Divide(2), 5).unwrap());
    }

    #[test]
    fn cage_is_valid_same_row_add_rejects_double() {
        // Sum 2 requires [1,1] which is filtered by row collinearity.
        let cs = pair_cells();
        assert!(!Polyomino::is_valid_operation(&cs, Operation::Add(2), 4).unwrap());
        assert!(Polyomino::is_valid_operation(&cs, Operation::Add(3), 4).unwrap());
    }

    #[test]
    fn cage_is_valid_l_shape_add_accepts_double() {
        // L-shape: (0,0) and (1,0) share a column but not a row with (0,1),
        // so [1,2,1] (sum=4) is legal — the repeated 1 is only across a non-collinear pair.
        assert!(Polyomino::is_valid_operation(&l_shape_cells(), Operation::Add(4), 4).unwrap());
    }

    #[test]
    fn cage_is_valid_disconnected_cells_returns_err() {
        assert!(matches!(
            Polyomino::is_valid_operation(&disconnected_cells(), Operation::Add(3), 4),
            Err(Error::DisconnectedPolyomino)
        ));
    }

    #[test]
    fn cage_is_valid_three_cells_rejects_subtract_and_divide() {
        let cs = row3_cells();
        assert!(!Polyomino::is_valid_operation(&cs, Operation::Subtract(1), 5).unwrap());
        assert!(!Polyomino::is_valid_operation(&cs, Operation::Divide(2), 5).unwrap());
        assert!(Polyomino::is_valid_operation(&cs, Operation::Add(6), 5).unwrap());
        assert!(Polyomino::is_valid_operation(&cs, Operation::Multiply(6), 5).unwrap());
    }

    #[test]
    fn valid_targets_invalid_operator_for_shape_is_empty() {
        // Given is only valid for 1-cell cages; called on a 2-cell shape, returns empty.
        let got: Vec<Operation> = Polyomino::valid_operations(&pair_cells(), Operator::Given, 4)
            .unwrap()
            .collect();
        assert!(got.is_empty());
    }

    #[test]
    fn valid_targets_disconnected_cells_returns_err() {
        assert!(matches!(
            Polyomino::valid_operations(&disconnected_cells(), Operator::Add, 4),
            Err(Error::DisconnectedPolyomino)
        ));
    }

    #[test]
    fn valid_targets_given_singleton_enumerates_one_through_n() {
        let got: Vec<Operation> =
            Polyomino::valid_operations(&singleton_cells(), Operator::Given, 4)
                .unwrap()
                .collect();
        assert_eq!(
            got,
            vec![
                Operation::Given(1),
                Operation::Given(2),
                Operation::Given(3),
                Operation::Given(4)
            ]
        );
    }

    #[test]
    fn valid_targets_subtract_pair_enumerates_one_through_n_minus_one() {
        let got: Vec<Operation> = Polyomino::valid_operations(&pair_cells(), Operator::Subtract, 4)
            .unwrap()
            .collect();
        assert_eq!(
            got,
            vec![
                Operation::Subtract(1),
                Operation::Subtract(2),
                Operation::Subtract(3)
            ]
        );
    }

    #[test]
    fn valid_targets_divide_pair_enumerates_two_through_n() {
        let got: Vec<Operation> = Polyomino::valid_operations(&pair_cells(), Operator::Divide, 4)
            .unwrap()
            .collect();
        assert_eq!(
            got,
            vec![
                Operation::Divide(2),
                Operation::Divide(3),
                Operation::Divide(4)
            ]
        );
    }

    #[test]
    fn valid_targets_add_pair_filters_by_admissibility() {
        // n=4, 2-cell same row: Add(2) requires [1,1] which violates collinearity.
        let got: Vec<Operation> = Polyomino::valid_operations(&pair_cells(), Operator::Add, 4)
            .unwrap()
            .collect();
        assert!(!got.contains(&Operation::Add(2)));
        assert!(got.contains(&Operation::Add(3)));
        assert!(got.contains(&Operation::Add(7)));
    }

    #[test]
    fn valid_targets_multiply_pair_filters_by_admissibility() {
        // n=4, 2-cell same row: Multiply(1) requires [1,1] which violates collinearity.
        let got: Vec<Operation> = Polyomino::valid_operations(&pair_cells(), Operator::Multiply, 4)
            .unwrap()
            .collect();
        assert!(!got.contains(&Operation::Multiply(1)));
        assert!(got.contains(&Operation::Multiply(2)));
    }

    // --- collinear_pairs ---

    #[test]
    fn collinear_pairs_single_cell_is_empty() {
        assert!(singleton().collinear_pairs().is_empty());
    }

    #[test]
    fn collinear_pairs_same_row() {
        let mut pairs = row3().collinear_pairs();
        pairs.sort_unstable();
        assert_eq!(pairs, vec![(0, 1), (0, 2), (1, 2)]);
    }

    #[test]
    fn collinear_pairs_same_column() {
        let mut pairs = col_pair().collinear_pairs();
        pairs.sort_unstable();
        assert_eq!(pairs, vec![(0, 1)]);
    }

    #[test]
    fn collinear_pairs_l_shape() {
        // (0,0), (1,0), (1,1): col-0 gives (0,1); row-1 gives (1,2).
        let mut pairs = l_shape().collinear_pairs();
        pairs.sort_unstable();
        assert_eq!(pairs, vec![(0, 1), (1, 2)]);
    }

    // --- operator_tuples ---

    #[test]
    fn operator_tuples_given_singleton() {
        let map = singleton().operator_tuples(4, Operator::Given);
        assert_eq!(map.len(), 4);
        assert_eq!(map[&Operation::Given(1)], vec![vec![1]]);
        assert_eq!(map[&Operation::Given(4)], vec![vec![4]]);
    }

    #[test]
    fn operator_tuples_given_non_singleton_is_empty() {
        assert!(pair().operator_tuples(4, Operator::Given).is_empty());
    }

    #[test]
    fn operator_tuples_subtract_non_pair_is_empty() {
        assert!(row3().operator_tuples(4, Operator::Subtract).is_empty());
    }

    #[test]
    fn operator_tuples_subtract_pair_same_row() {
        // Same row: diff=1 → both orderings [1,2] and [2,1].
        let map = pair().operator_tuples(4, Operator::Subtract);
        let mut t = map[&Operation::Subtract(1)].clone();
        t.sort();
        assert!(t.contains(&vec![1, 2]));
        assert!(t.contains(&vec![2, 1]));
    }

    #[test]
    fn operator_tuples_add_same_row_excludes_doubles() {
        // n=4, same-row 2-cell: Add(2) requires [1,1] which violates collinearity.
        let map = pair().operator_tuples(4, Operator::Add);
        assert!(!map.contains_key(&Operation::Add(2)));
        assert!(map.contains_key(&Operation::Add(3)));
    }

    #[test]
    fn operator_tuples_multiply_same_row() {
        let map = pair().operator_tuples(4, Operator::Multiply);
        // 1*4, 2*3, etc. allowed; squares (1*1, 2*2, 3*3) filtered by row collinearity.
        assert!(map.contains_key(&Operation::Multiply(6)));
        assert!(map.contains_key(&Operation::Multiply(4))); // 1*4, not 2*2
        assert!(!map.contains_key(&Operation::Multiply(1))); // only 1*1, filtered
        assert!(!map.contains_key(&Operation::Multiply(9))); // only 3*3, filtered
    }

    #[test]
    fn operator_tuples_divide_non_pair_is_empty() {
        assert!(row3().operator_tuples(4, Operator::Divide).is_empty());
    }

    #[test]
    fn operator_tuples_divide_same_row_pair() {
        // Same-row pair on n=4: values must differ, so [2,2] is pruned.
        // Divide(2): [1,2],[2,1],[2,4],[4,2]; Divide(3): [1,3],[3,1]; Divide(4): [1,4],[4,1]
        let tuples = pair().operator_tuples(4, Operator::Divide);
        let mut got: Vec<Vec<u8>> = tuples.values().flat_map(|ts| ts.iter().cloned()).collect();
        got.sort_unstable();
        assert_eq!(
            got,
            vec![
                vec![1u8, 2],
                vec![1, 3],
                vec![1, 4],
                vec![2, 1],
                vec![2, 4],
                vec![3, 1],
                vec![4, 1],
                vec![4, 2],
            ]
        );
    }

    #[test]
    fn operator_tuples_add_singleton_is_empty() {
        assert!(singleton().operator_tuples(4, Operator::Add).is_empty());
    }

    #[test]
    fn operator_tuples_multiply_singleton_is_empty() {
        assert!(
            singleton()
                .operator_tuples(4, Operator::Multiply)
                .is_empty()
        );
    }

    // --- Polyomino::insert ---

    #[test]
    fn insert_adds_adjacent_cell() {
        let p2 = singleton().insert(c01()).unwrap();
        assert!(p2.cells().any(|cell| cell == c01()));
        assert_eq!(p2.len(), 2);
    }

    #[test]
    fn insert_is_idempotent() {
        assert_eq!(pair().insert(c00()).unwrap().len(), 2);
    }

    #[test]
    fn insert_disconnected_returns_err() {
        assert!(matches!(
            singleton().insert(c02()),
            Err(Error::DisconnectedPolyomino)
        ));
    }

    #[test]
    fn insert_result_is_sorted() {
        let p = Polyomino::from_cells(&[c01()]).unwrap();
        let p2 = p.insert(c00()).unwrap();
        assert_eq!(p2.cells().next(), Some(c00()));
    }

    // --- Polyomino::intersects ---

    #[test]
    fn intersects_disjoint_returns_false() {
        assert!(!singleton().intersects(&pair().remove(c00()).unwrap()));
    }

    #[test]
    fn intersects_shared_cell_returns_true() {
        assert!(pair().intersects(&col_pair()));
    }

    #[test]
    fn intersects_same_polyomino_returns_true() {
        assert!(pair().intersects(&pair()));
    }

    #[test]
    fn intersects_is_symmetric() {
        let a = pair();
        let b = col_pair();
        assert_eq!(a.intersects(&b), b.intersects(&a));
    }

    // --- Polyomino::remove ---

    #[test]
    fn remove_deletes_cell() {
        let p2 = pair().remove(c01()).unwrap();
        assert!(!p2.cells().any(|cell| cell == c01()));
        assert_eq!(p2.len(), 1);
    }

    #[test]
    fn remove_is_idempotent() {
        let p = pair();
        assert_eq!(p.remove(c11()).unwrap(), p);
    }

    #[test]
    fn remove_last_cell_returns_err() {
        assert!(matches!(
            singleton().remove(c00()),
            Err(Error::EmptyPolyomino)
        ));
    }
}
