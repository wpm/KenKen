use std::collections::HashMap;

use crate::{
    Error, Fill, Operation, Operator,
    constraints::{
        Constraint, RegionConstraint,
        cage::arithmetic::{
            addition_multisets, division_multisets, multiplication_multisets, subtraction_multisets,
        },
        cover::{Cover, Polyomino},
    },
    grid::Grid,
    types::{Cell, Index, M, N},
};

/// An ordered assignment of values to the cells of a cage, one value per cell.
pub type Tuple = Vec<N>;

/// A polyomino constraint defined by a set of cells and an arithmetic
/// operation.
///
/// Stores the valid ordered tuples for the operation after filtering out
/// assignments that repeat a value within any shared row or column of the
/// polyomino.
#[must_use]
#[derive(Debug, Clone)]
pub struct Cage {
    polyomino: Polyomino,
    operation: Operation,
    tuples: Vec<Tuple>,
}

impl Cage {
    /// Creates a cage over the given polyomino. Stores valid ordered tuples for
    /// `operation`, with tuples that repeat a value within any shared row
    /// or column dropped.
    pub fn new(n: N, polyomino: Polyomino, operation: Operation) -> Self {
        let tuples = operation_tuples(n, &polyomino, operation);
        Self {
            polyomino,
            operation,
            tuples,
        }
    }

    /// Returns the polyomino covered by this cage.
    pub const fn polyomino(&self) -> &Polyomino {
        &self.polyomino
    }

    /// Returns this cage's operation.
    #[must_use]
    pub const fn operation(&self) -> Operation {
        self.operation
    }

    /// Returns a slice over the cage's precomputed valid ordered tuples.
    #[must_use]
    pub fn tuples(&self) -> &[Tuple] {
        &self.tuples
    }

    /// Returns the operators legal for a cage covering `cells`.
    ///
    /// Singleton cages permit only [`Operator::Given`]; 2-cell cages permit all
    /// operators; larger cages permit only [`Operator::Add`] and
    /// [`Operator::Multiply`].
    #[must_use]
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
    pub fn valid_targets(
        cells: &[Cell],
        op: Operator,
        n: N,
    ) -> Result<Box<dyn Iterator<Item = Operation>>, Error> {
        if !Self::valid_operators(cells).contains(&op) {
            return Ok(Box::new(std::iter::empty()));
        }
        let polyomino = Polyomino::new(cells)?;
        let k = polyomino.len();
        let n_m = M::from(n);
        Ok(match op {
            Operator::Given => Box::new((1..=n_m).map(Operation::Given)),
            Operator::Subtract => Box::new((1..=n_m.saturating_sub(1)).map(Operation::Subtract)),
            Operator::Divide => Box::new((2..=n_m).map(Operation::Divide)),
            Operator::Add => {
                let max = M::try_from(usize::from(n).saturating_mul(k)).unwrap_or(M::MAX);
                Box::new((1..=max).filter_map(move |t| {
                    let op = Operation::Add(t);
                    has_admissible_tuple(n, &polyomino, op).then_some(op)
                }))
            }
            Operator::Multiply => {
                let exp = u32::try_from(k).unwrap_or(u32::MAX);
                let max = n_m.saturating_pow(exp);
                Box::new((1..=max).filter_map(move |t| {
                    let op = Operation::Multiply(t);
                    has_admissible_tuple(n, &polyomino, op).then_some(op)
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
    pub fn is_valid(cells: &[Cell], operation: Operation, n: N) -> Result<bool, Error> {
        if !Self::valid_operators(cells).contains(&Operator::of(operation)) {
            return Ok(false);
        }
        match operation {
            Operation::Given(v) => return Ok((1..=M::from(n)).contains(&v)),
            Operation::Subtract(0) | Operation::Divide(0 | 1) => return Ok(false),
            _ => {}
        }
        let polyomino = Polyomino::new(cells)?;
        Ok(has_admissible_tuple(n, &polyomino, operation))
    }
}

/// Returns true if `operation` has at least one valid tuple for the given
/// polyomino.
fn has_admissible_tuple(n: N, polyomino: &Polyomino, operation: Operation) -> bool {
    operator_tuples(n, polyomino, Operator::of(operation)).contains_key(&operation)
}

impl Cover for Cage {
    fn cells(&self) -> Vec<Cell> {
        self.polyomino.cells()
    }

    fn len(&self) -> usize {
        self.polyomino.len()
    }
}

impl RegionConstraint for Cage {
    fn constraint(&self, _grid: &Grid) -> Constraint {
        let n = self.len();
        let mut cols = vec![Fill::default(); n];
        for tuple in self.tuples() {
            for (col, &val) in cols.iter_mut().zip(tuple.iter()) {
                *col = *col | Fill::new([val]);
            }
        }
        self.cells().iter().copied().zip(cols).collect()
    }
}

/// Returns a map from each valid [`Operation`] to the ordered tuples that
/// realize it, for the given operator applied to the polyomino on an `n`×`n`
/// grid.
///
/// Each key is an `(operator, target)` pair for which at least one assignment
/// of grid values to the polyomino's cells satisfies the operation and the
/// collinearity constraints.
///
/// Subtract and Divide are only valid for 2-cell polyominoes; any other size
/// yields an empty map.
#[must_use]
pub fn operator_tuples(
    n: N,
    polyomino: &Polyomino,
    operator: Operator,
) -> HashMap<Operation, Vec<Tuple>> {
    let k = polyomino.len();
    let pairs = collinear_pairs(polyomino);

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

/// Returns the valid ordered tuples for a single known `operation` on the given
/// polyomino.
///
/// Unlike [`operator_tuples`], which enumerates all targets for an operator,
/// this function uses the target already encoded in `operation` and generates
/// only its tuples.
fn operation_tuples(n: N, polyomino: &Polyomino, operation: Operation) -> Vec<Tuple> {
    let k = polyomino.len();
    let pairs = collinear_pairs(polyomino);
    match operation {
        Operation::Given(v) => {
            N::try_from(v).map_or_else(|_| vec![], |v_n| ordered_tuples(&[v_n], &pairs).collect())
        }
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

/// Returns pairs of cell indices within `cells` that share a row or column.
///
/// Each pair `(i, j)` with `i < j` means `cells[i]` and `cells[j]` must hold
/// distinct values.
fn collinear_pairs(polyomino: &Polyomino) -> Vec<(usize, usize)> {
    let mut by_row: HashMap<Index, Vec<usize>> = HashMap::new();
    let mut by_col: HashMap<Index, Vec<usize>> = HashMap::new();
    for (i, cell) in polyomino.cells().iter().enumerate() {
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

/// Returns an iterator over all ordered permutations of `multiset` that satisfy
/// the collinearity constraint: for every `(i, j)` in `pairs`, the values at
/// positions `i` and `j` differ.
fn ordered_tuples<'a>(
    multiset: &'a [N],
    pairs: &'a [(usize, usize)],
) -> impl Iterator<Item = Tuple> + 'a {
    permutations(multiset).filter(move |t| pairs.iter().all(|&(i, j)| t[i] != t[j]))
}

/// Returns an iterator over all distinct permutations of `values` in
/// lexicographic order.
fn permutations(values: &[N]) -> impl Iterator<Item = Tuple> {
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::types::Cell;

    fn cells(positions: &[(usize, usize)]) -> Vec<Cell> {
        positions.iter().map(|&(r, c)| Cell::new(r, c)).collect()
    }

    // --- collinear_pairs ---

    #[test]
    fn collinear_pairs_single_cell_is_empty() {
        let positions = &[(0, 0)];
        assert!(collinear_pairs(&Polyomino::new(&cells(positions)).unwrap()).is_empty());
    }

    #[test]
    fn collinear_pairs_same_row() {
        let positions = &[(0, 0), (0, 1), (0, 2)];
        let mut pairs = collinear_pairs(&Polyomino::new(&cells(positions)).unwrap());
        pairs.sort_unstable();
        assert_eq!(pairs, vec![(0, 1), (0, 2), (1, 2)]);
    }

    #[test]
    fn collinear_pairs_same_column() {
        let positions = &[(0, 0), (1, 0)];
        let mut pairs = collinear_pairs(&Polyomino::new(&cells(positions)).unwrap());
        pairs.sort_unstable();
        assert_eq!(pairs, vec![(0, 1)]);
    }

    #[test]
    fn collinear_pairs_l_shape() {
        // (0,0), (1,0), (1,1): col-0 gives (0,1); row-1 gives (1,2).
        let positions = &[(0, 0), (1, 0), (1, 1)];
        let mut pairs = collinear_pairs(&Polyomino::new(&cells(positions)).unwrap());
        pairs.sort_unstable();
        assert_eq!(pairs, vec![(0, 1), (1, 2)]);
    }

    // --- permutations / next_permutation ---

    #[test]
    fn permutations_single_element() {
        assert_eq!(permutations(&[1]).collect::<Vec<_>>(), vec![vec![1]]);
    }

    #[test]
    fn permutations_two_distinct_elements() {
        assert_eq!(
            permutations(&[1, 2]).collect::<Vec<_>>(),
            vec![vec![1, 2], vec![2, 1]]
        );
    }

    #[test]
    fn permutations_two_equal_elements() {
        assert_eq!(permutations(&[2, 2]).collect::<Vec<_>>(), vec![vec![2, 2]]);
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
        // Diagonal pair: no collinearity constraint.
        let pairs = vec![];
        let result: Vec<Tuple> = ordered_tuples(&[1, 2], &pairs).collect();
        assert_eq!(result, vec![vec![1, 2], vec![2, 1]]);
    }

    #[test]
    fn ordered_tuples_same_row_filters_equal_values() {
        // Row pair: (0,1) must differ.
        let pairs = vec![(0, 1)];
        assert!(ordered_tuples(&[2, 2], &pairs).next().is_none());
    }

    // --- operator_tuples ---

    #[test]
    fn operator_tuples_given_singleton() {
        let positions = &[(0, 0)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        let map = operator_tuples(4, &p, Operator::Given);
        assert_eq!(map.len(), 4);
        assert_eq!(map[&Operation::Given(1)], vec![vec![1]]);
        assert_eq!(map[&Operation::Given(4)], vec![vec![4]]);
    }

    #[test]
    fn operator_tuples_given_non_singleton_is_empty() {
        let positions = &[(0, 0), (0, 1)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        assert!(operator_tuples(4, &p, Operator::Given).is_empty());
    }

    #[test]
    fn operator_tuples_subtract_non_pair_is_empty() {
        let positions = &[(0, 0), (0, 1), (0, 2)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        assert!(operator_tuples(4, &p, Operator::Subtract).is_empty());
    }

    #[test]
    fn operator_tuples_subtract_pair_same_row() {
        // Same row: (1,2) gives diff=1 → both orderings [1,2] and [2,1].
        let positions = &[(0, 0), (0, 1)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        let map = operator_tuples(4, &p, Operator::Subtract);
        let mut t = map[&Operation::Subtract(1)].clone();
        t.sort();
        assert!(t.contains(&vec![1, 2]));
        assert!(t.contains(&vec![2, 1]));
    }

    #[test]
    fn operator_tuples_add_same_row_excludes_doubles() {
        // n=4, same-row 2-cell: Add(2) requires [1,1] which violates collinearity.
        let positions = &[(0, 0), (0, 1)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        let map = operator_tuples(4, &p, Operator::Add);
        assert!(!map.contains_key(&Operation::Add(2)));
        assert!(map.contains_key(&Operation::Add(3)));
    }

    #[test]
    fn operator_tuples_multiply_same_row() {
        let positions = &[(0, 0), (0, 1)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        let map = operator_tuples(4, &p, Operator::Multiply);
        // 1*4, 2*3, etc. allowed; squares (1*1, 2*2, 3*3) filtered by row collinearity.
        assert!(map.contains_key(&Operation::Multiply(6)));
        assert!(map.contains_key(&Operation::Multiply(4))); // 1*4, not 2*2
        assert!(!map.contains_key(&Operation::Multiply(1))); // only 1*1, filtered
        assert!(!map.contains_key(&Operation::Multiply(9))); // only 3*3, filtered
    }

    #[test]
    fn operator_tuples_divide_non_pair_is_empty() {
        let positions = &[(0, 0), (0, 1), (0, 2)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        assert!(operator_tuples(4, &p, Operator::Divide).is_empty());
    }

    // --- Cage::new ---

    #[test]
    fn cage_new_given_singleton() {
        let positions = &[(0, 0)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        let cage = Cage::new(4, p, Operation::Given(3));
        assert_eq!(cage.tuples(), &[vec![3u8]]);
    }

    #[test]
    fn cage_new_subtract_same_row_pair() {
        // Same-row pair: both orderings still survive since values differ.
        let positions = &[(0, 0), (0, 1)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        let cage = Cage::new(4, p, Operation::Subtract(1));
        let mut tuples = cage.tuples().to_vec();
        tuples.sort_unstable();
        // Subtract(1): multisets [1,2],[2,3],[3,4] → both orderings each.
        assert_eq!(
            tuples,
            vec![
                vec![1u8, 2],
                vec![2, 1],
                vec![2, 3],
                vec![3, 2],
                vec![3, 4],
                vec![4, 3],
            ]
        );
    }

    #[test]
    fn cage_new_divide_same_row_pair() {
        // Divide(2) on a same-row pair: [1,2] and [2,4] → both orderings each.
        let positions = &[(0, 0), (0, 1)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        let cage = Cage::new(4, p, Operation::Divide(2));
        let mut tuples = cage.tuples().to_vec();
        tuples.sort_unstable();
        assert_eq!(
            tuples,
            vec![vec![1u8, 2], vec![2, 1], vec![2, 4], vec![4, 2]]
        );
    }

    #[test]
    fn cage_new_multiply_same_row_pair() {
        // Multiply(6) on a same-row pair: multisets [1,6] and [2,3] → both orderings
        // each.
        let positions = &[(0, 0), (0, 1)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        let cage = Cage::new(6, p, Operation::Multiply(6));
        let mut tuples = cage.tuples().to_vec();
        tuples.sort_unstable();
        assert_eq!(
            tuples,
            vec![vec![1u8, 6], vec![2, 3], vec![3, 2], vec![6, 1]]
        );
    }

    #[test]
    fn cage_new_add_prunes_horizontal_pair() {
        // Add(6) on same-row pair: [2,4],[4,2],[3,3] — [3,3] filtered by row
        // collinearity.
        let positions = &[(0, 0), (0, 1)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        let cage = Cage::new(4, p, Operation::Add(6));
        let mut tuples = cage.tuples().to_vec();
        tuples.sort_unstable();
        assert_eq!(tuples, vec![vec![2u8, 4], vec![4, 2]]);
    }

    #[test]
    fn cage_new_add_prunes_l_shape() {
        // (0,0),(1,0),(1,1): col-0 pair (0,1), row-1 pair (1,2).
        // 10 raw permutations from {1,1,4},{1,2,3},{2,2,2} → 7 survive.
        let positions = &[(0, 0), (1, 0), (1, 1)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        let cage = Cage::new(4, p, Operation::Add(6));
        assert_eq!(cage.tuples().len(), 7);
        assert!(!cage.tuples().contains(&vec![1u8, 1, 4]));
        assert!(!cage.tuples().contains(&vec![2, 2, 2]));
    }

    #[test]
    fn cage_valid_operators_singleton() {
        assert_eq!(
            Cage::valid_operators(&cells(&[(0, 0)])),
            vec![Operator::Given]
        );
    }

    #[test]
    fn cage_valid_operators_two_cells() {
        assert_eq!(
            Cage::valid_operators(&cells(&[(0, 0), (0, 1)])),
            vec![
                Operator::Add,
                Operator::Subtract,
                Operator::Multiply,
                Operator::Divide,
            ]
        );
    }

    #[test]
    fn cage_valid_operators_three_cells() {
        assert_eq!(
            Cage::valid_operators(&cells(&[(0, 0), (0, 1), (0, 2)])),
            vec![Operator::Add, Operator::Multiply]
        );
    }

    #[test]
    fn cage_is_valid_singleton_given_in_range() {
        let cells = cells(&[(0, 0)]);
        assert!(Cage::is_valid(&cells, Operation::Given(1), 5).unwrap());
        assert!(Cage::is_valid(&cells, Operation::Given(5), 5).unwrap());
        assert!(!Cage::is_valid(&cells, Operation::Given(0), 5).unwrap());
        assert!(!Cage::is_valid(&cells, Operation::Given(6), 5).unwrap());
    }

    #[test]
    fn cage_is_valid_two_cell_subtract_zero_rejected() {
        let cs = cells(&[(0, 0), (0, 1)]);
        assert!(!Cage::is_valid(&cs, Operation::Subtract(0), 5).unwrap());
        assert!(Cage::is_valid(&cs, Operation::Subtract(1), 5).unwrap());
    }

    #[test]
    fn cage_is_valid_two_cell_divide_below_two_rejected() {
        let cs = cells(&[(0, 0), (0, 1)]);
        assert!(!Cage::is_valid(&cs, Operation::Divide(0), 5).unwrap());
        assert!(!Cage::is_valid(&cs, Operation::Divide(1), 5).unwrap());
        assert!(Cage::is_valid(&cs, Operation::Divide(2), 5).unwrap());
    }

    #[test]
    fn cage_is_valid_same_row_add_rejects_double() {
        // Sum 2 requires [1,1] which is filtered by row collinearity.
        let cs = cells(&[(0, 0), (0, 1)]);
        assert!(!Cage::is_valid(&cs, Operation::Add(2), 4).unwrap());
        assert!(Cage::is_valid(&cs, Operation::Add(3), 4).unwrap());
    }

    #[test]
    fn cage_is_valid_l_shape_add_accepts_double() {
        // L-shape: (0,0) and (1,0) share a column but not a row with (0,1),
        // so [1,2,1] (sum=4) is legal — the repeated 1 is only across a non-collinear
        // pair.
        let cs = cells(&[(0, 0), (0, 1), (1, 0)]);
        assert!(Cage::is_valid(&cs, Operation::Add(4), 4).unwrap());
    }

    #[test]
    fn cage_is_valid_disconnected_cells_returns_err() {
        let cs = cells(&[(0, 0), (1, 1)]);
        assert!(matches!(
            Cage::is_valid(&cs, Operation::Add(3), 4),
            Err(Error::DisconnectedPolyomino)
        ));
    }

    #[test]
    fn valid_operators_empty_cells_is_empty() {
        assert!(Cage::valid_operators(&[]).is_empty());
    }

    #[test]
    fn valid_targets_invalid_operator_for_shape_is_empty() {
        // Given is only valid for 1-cell cages; called on a 2-cell shape, returns
        // empty.
        let cs = cells(&[(0, 0), (0, 1)]);
        let got: Vec<Operation> = Cage::valid_targets(&cs, Operator::Given, 4)
            .unwrap()
            .collect();
        assert!(got.is_empty());
    }

    #[test]
    fn valid_targets_disconnected_cells_returns_err() {
        let cs = cells(&[(0, 0), (1, 1)]);
        assert!(matches!(
            Cage::valid_targets(&cs, Operator::Add, 4),
            Err(Error::DisconnectedPolyomino)
        ));
    }

    #[test]
    fn valid_targets_given_singleton_enumerates_one_through_n() {
        let cs = cells(&[(0, 0)]);
        let got: Vec<Operation> = Cage::valid_targets(&cs, Operator::Given, 4)
            .unwrap()
            .collect();
        assert_eq!(
            got,
            vec![
                Operation::Given(1),
                Operation::Given(2),
                Operation::Given(3),
                Operation::Given(4),
            ]
        );
    }

    #[test]
    fn valid_targets_subtract_pair_enumerates_one_through_n_minus_one() {
        let cs = cells(&[(0, 0), (0, 1)]);
        let got: Vec<Operation> = Cage::valid_targets(&cs, Operator::Subtract, 4)
            .unwrap()
            .collect();
        assert_eq!(
            got,
            vec![
                Operation::Subtract(1),
                Operation::Subtract(2),
                Operation::Subtract(3),
            ]
        );
    }

    #[test]
    fn valid_targets_divide_pair_enumerates_two_through_n() {
        let cs = cells(&[(0, 0), (0, 1)]);
        let got: Vec<Operation> = Cage::valid_targets(&cs, Operator::Divide, 4)
            .unwrap()
            .collect();
        assert_eq!(
            got,
            vec![
                Operation::Divide(2),
                Operation::Divide(3),
                Operation::Divide(4),
            ]
        );
    }

    #[test]
    fn valid_targets_add_pair_filters_by_admissibility() {
        // n=4, 2-cell same row: Add(2) requires [1,1] which violates collinearity.
        let cs = cells(&[(0, 0), (0, 1)]);
        let got: Vec<Operation> = Cage::valid_targets(&cs, Operator::Add, 4)
            .unwrap()
            .collect();
        assert!(!got.contains(&Operation::Add(2)));
        assert!(got.contains(&Operation::Add(3)));
        assert!(got.contains(&Operation::Add(7)));
    }

    #[test]
    fn valid_targets_multiply_pair_filters_by_admissibility() {
        // n=4, 2-cell same row: Multiply(1) requires [1,1] which violates collinearity.
        let cs = cells(&[(0, 0), (0, 1)]);
        let got: Vec<Operation> = Cage::valid_targets(&cs, Operator::Multiply, 4)
            .unwrap()
            .collect();
        assert!(!got.contains(&Operation::Multiply(1)));
        assert!(got.contains(&Operation::Multiply(2)));
    }

    #[test]
    fn cage_cells_via_trait_returns_same_as_inherent() {
        let positions = &[(0, 0), (0, 1)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        let cage = Cage::new(4, p, Operation::Add(3));
        let via_trait: Vec<Cell> = <Cage as Cover>::cells(&cage);
        assert_eq!(via_trait, cage.cells());
    }

    #[test]
    fn operator_tuples_add_singleton_is_empty() {
        let positions = &[(0, 0)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        assert!(operator_tuples(4, &p, Operator::Add).is_empty());
    }

    #[test]
    fn operator_tuples_multiply_singleton_is_empty() {
        let positions = &[(0, 0)];
        let p = Polyomino::new(&cells(positions)).unwrap();
        assert!(operator_tuples(4, &p, Operator::Multiply).is_empty());
    }

    #[test]
    fn cage_new_with_target_above_n_max_yields_no_tuples() {
        let singleton = || {
            let positions = &[(0, 0)];
            Polyomino::new(&cells(positions)).unwrap()
        };
        let pair = || {
            let positions = &[(0, 0), (0, 1)];
            Polyomino::new(&cells(positions)).unwrap()
        };
        for (p, op) in [
            (singleton(), Operation::Given(300)),
            (pair(), Operation::Subtract(300)),
            (pair(), Operation::Divide(300)),
            (pair(), Operation::Add(300)),
        ] {
            assert!(Cage::new(4, p, op).tuples().is_empty());
        }
    }

    #[test]
    fn cage_is_valid_three_cells_rejects_subtract_and_divide() {
        let cs = cells(&[(0, 0), (0, 1), (0, 2)]);
        assert!(!Cage::is_valid(&cs, Operation::Subtract(1), 5).unwrap());
        assert!(!Cage::is_valid(&cs, Operation::Divide(2), 5).unwrap());
        assert!(Cage::is_valid(&cs, Operation::Add(6), 5).unwrap());
        assert!(Cage::is_valid(&cs, Operation::Multiply(6), 5).unwrap());
    }
}
