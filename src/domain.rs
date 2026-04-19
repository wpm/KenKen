use crate::geometry::conflict_graph;
use crate::types::{Cage, Operation, Tuple, Value};

/// Enumerates all valid tuples for a cage in an n×n grid.
///
/// A tuple is valid if:
/// 1. The cage operation is satisfied by the tuple values.
/// 2. For every (i, j) in conflict_graph(cage), tuple[i] != tuple[j].
pub fn valid_tuples(cage: &Cage, n: usize) -> Vec<Tuple> {
    let k = cage.cells.len();
    let conflicts = conflict_graph(cage);
    let mut results = Vec::new();
    enumerate(
        k,
        n,
        &cage.op,
        &mut Vec::with_capacity(k),
        &conflicts,
        &mut results,
    );
    results
}

/// Recursively enumerates all (1..=n)^k assignments, pruning on conflicts and
/// collecting those that also satisfy the operation.
fn enumerate(
    k: usize,
    n: usize,
    op: &Operation,
    current: &mut Vec<Value>,
    conflicts: &[(usize, usize)],
    results: &mut Vec<Tuple>,
) {
    let pos = current.len();
    if pos == k {
        if satisfies_operation(op, current) {
            results.push(current.clone());
        }
        return;
    }
    'outer: for v in 1..=(n as Value) {
        for &(i, j) in conflicts {
            if j == pos && current[i] == v {
                continue 'outer;
            }
        }
        current.push(v);
        enumerate(k, n, op, current, conflicts, results);
        current.pop();
    }
}

pub(crate) fn satisfies_operation(op: &Operation, tuple: &[Value]) -> bool {
    match op {
        Operation::Given(v) => tuple.len() == 1 && tuple[0] == *v,
        Operation::Add(t) => {
            let sum: u32 = tuple.iter().map(|&v| v as u32).sum();
            sum == *t
        }
        Operation::Sub(t) => {
            tuple.len() == 2 && (tuple[0] as i32 - tuple[1] as i32).unsigned_abs() == *t
        }
        Operation::Mul(t) => {
            let prod: u32 = tuple.iter().map(|&v| v as u32).product();
            prod == *t
        }
        Operation::Div(t) => {
            if tuple.len() != 2 {
                return false;
            }
            let a = tuple[0] as u32;
            let b = tuple[1] as u32;
            let (big, small) = if a >= b { (a, b) } else { (b, a) };
            small != 0 && big % small == 0 && big / small == *t
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Operation;

    fn make_cage(cells: Vec<(usize, usize)>, op: Operation) -> Cage {
        Cage { cells, op }
    }

    #[test]
    fn sub_cage_4x4() {
        // 2-cell Sub(1) cage in 4×4; cells in different rows and columns so no conflict.
        let cage = make_cage(vec![(0, 0), (1, 1)], Operation::Sub(1));
        let mut tuples = valid_tuples(&cage, 4);
        tuples.sort();
        let expected: Vec<Tuple> = vec![
            vec![1, 2],
            vec![2, 1],
            vec![2, 3],
            vec![3, 2],
            vec![3, 4],
            vec![4, 3],
        ];
        assert_eq!(tuples, expected);
    }

    #[test]
    fn div_cage_4x4() {
        // 2-cell Div(2) cage in 4×4; cells in different rows and columns.
        let cage = make_cage(vec![(0, 0), (1, 1)], Operation::Div(2));
        let mut tuples = valid_tuples(&cage, 4);
        tuples.sort();
        let expected: Vec<Tuple> = vec![vec![1, 2], vec![2, 1], vec![2, 4], vec![4, 2]];
        assert_eq!(tuples, expected);
    }

    #[test]
    fn given_cage() {
        // 1-cell Given(3) cage in 4×4.
        let cage = make_cage(vec![(0, 0)], Operation::Given(3));
        let tuples = valid_tuples(&cage, 4);
        assert_eq!(tuples, vec![vec![3]]);
    }

    #[test]
    fn add_cage_no_conflict() {
        // 2-cell Add(5) cage where cells don't share row or column.
        let cage = make_cage(vec![(0, 0), (1, 1)], Operation::Add(5));
        let mut tuples = valid_tuples(&cage, 4);
        tuples.sort();
        let expected: Vec<Tuple> = vec![vec![1, 4], vec![2, 3], vec![3, 2], vec![4, 1]];
        assert_eq!(tuples, expected);
    }

    #[test]
    fn add_cage_with_conflict() {
        // 2-cell Add(4) cage where cells share row 0 → conflict excludes [2,2].
        let cage = make_cage(vec![(0, 0), (0, 1)], Operation::Add(4));
        let mut tuples = valid_tuples(&cage, 4);
        tuples.sort();
        // Pairs summing to 4 in 1..=4 are [1,3],[2,2],[3,1].
        // [2,2] excluded because cells share row (conflict).
        let expected: Vec<Tuple> = vec![vec![1, 3], vec![3, 1]];
        assert_eq!(tuples, expected);
    }
}
