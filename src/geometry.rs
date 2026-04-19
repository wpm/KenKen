use crate::types::{Cage, Cell, LatinSquare, Operation};
use std::collections::{HashMap, HashSet, VecDeque};

/// Returns n² singleton Given cages, one per cell, in row-major order.
pub fn trivial_cages(ls: &LatinSquare) -> Vec<Cage> {
    (0..ls.n)
        .flat_map(|r| {
            (0..ls.n).map(move |c| Cage {
                cells: vec![(r, c)],
                op: Operation::Given(ls.get((r, c))),
            })
        })
        .collect()
}

/// Returns true if the cage's cells form a 4-connected polyomino.
pub fn is_cage_contiguous(cage: &Cage) -> bool {
    if cage.cells.len() <= 1 {
        return true;
    }
    let cell_set: HashSet<Cell> = cage.cells.iter().copied().collect();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(cage.cells[0]);
    visited.insert(cage.cells[0]);
    while let Some((r, c)) = queue.pop_front() {
        for nb in [
            (r.wrapping_sub(1), c),
            (r + 1, c),
            (r, c.wrapping_sub(1)),
            (r, c + 1),
        ] {
            if cell_set.contains(&nb) && visited.insert(nb) {
                queue.push_back(nb);
            }
        }
    }
    visited.len() == cage.cells.len()
}

/// Returns all pairs (i,j) with i<j where cage i and cage j share a border
/// (4-connectivity). Result is sorted and deduplicated.
pub fn adjacent_pairs(cages: &[Cage]) -> Vec<(usize, usize)> {
    let mut cell_to_cage: HashMap<Cell, usize> = HashMap::new();
    for (idx, cage) in cages.iter().enumerate() {
        for &cell in &cage.cells {
            cell_to_cage.insert(cell, idx);
        }
    }

    let mut pairs: HashSet<(usize, usize)> = HashSet::new();
    for (idx, cage) in cages.iter().enumerate() {
        for &(r, c) in &cage.cells {
            let neighbors: [(usize, usize); 4] = [
                (r.wrapping_sub(1), c),
                (r + 1, c),
                (r, c.wrapping_sub(1)),
                (r, c + 1),
            ];
            for &neighbor in &neighbors {
                if let Some(&other) = cell_to_cage.get(&neighbor)
                    && other != idx
                {
                    pairs.insert((idx.min(other), idx.max(other)));
                }
            }
        }
    }

    let mut result: Vec<(usize, usize)> = pairs.into_iter().collect();
    result.sort_unstable();
    result
}

/// Merges two cages: cells = a.cells ++ b.cells, with the given operation.
pub fn merge_cages(a: &Cage, b: &Cage, op: Operation) -> Cage {
    Cage {
        cells: a.cells.iter().chain(b.cells.iter()).copied().collect(),
        op,
    }
}

/// Replaces cages[i] and cages[j] with merged, returning the new cage list.
pub fn replace_with_merged(cages: &[Cage], i: usize, j: usize, merged: Cage) -> Vec<Cage> {
    cages
        .iter()
        .enumerate()
        .filter(|&(k, _)| k != i && k != j)
        .map(|(_, c)| c.clone())
        .chain(std::iter::once(merged))
        .collect()
}

/// Returns all pairs of positions (i,j) with i<j in cage.cells that share a
/// row or column. Result is sorted.
pub fn conflict_graph(cage: &Cage) -> Vec<(usize, usize)> {
    let cells = &cage.cells;
    let mut pairs = Vec::new();
    for i in 0..cells.len() {
        for j in (i + 1)..cells.len() {
            if cells[i].0 == cells[j].0 || cells[i].1 == cells[j].1 {
                pairs.push((i, j));
            }
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::fixtures::make_2x2_all_given_puzzle;
    use crate::types::Operation;

    fn make_3x3_latin_square() -> LatinSquare {
        LatinSquare {
            n: 3,
            grid: vec![vec![1, 2, 3], vec![2, 3, 1], vec![3, 1, 2]],
        }
    }

    #[test]
    fn trivial_cages_count() {
        let ls = make_3x3_latin_square();
        let cages = trivial_cages(&ls);
        assert_eq!(cages.len(), 9);
        for cage in &cages {
            assert_eq!(cage.cells.len(), 1);
        }
    }

    #[test]
    fn trivial_cages_values() {
        let ls = make_3x3_latin_square();
        let cages = trivial_cages(&ls);
        // Row-major order: (0,0),(0,1),...,(2,2)
        for (idx, cage) in cages.iter().enumerate() {
            let r = idx / 3;
            let c = idx % 3;
            assert_eq!(cage.cells[0], (r, c));
            let expected_val = ls.get((r, c));
            assert_eq!(cage.op, Operation::Given(expected_val));
        }
    }

    #[test]
    fn adjacent_pairs_2x2() {
        // 4 singleton cages arranged in a 2x2 grid:
        // cage 0: (0,0), cage 1: (0,1), cage 2: (1,0), cage 3: (1,1)
        let cages = make_2x2_all_given_puzzle().cages;
        let pairs = adjacent_pairs(&cages);
        // (0,0)↔(0,1): (0,1); (0,0)↔(1,0): (0,2); (0,1)↔(1,1): (1,3); (1,0)↔(1,1): (2,3)
        assert_eq!(pairs, vec![(0, 1), (0, 2), (1, 3), (2, 3)]);
    }

    #[test]
    fn adjacent_pairs_no_diagonals() {
        // Two cages at diagonal positions only — should NOT be adjacent
        let cages = vec![
            Cage {
                cells: vec![(0, 0)],
                op: Operation::Given(1),
            },
            Cage {
                cells: vec![(1, 1)],
                op: Operation::Given(2),
            },
        ];
        let pairs = adjacent_pairs(&cages);
        assert!(pairs.is_empty(), "diagonal cages should not be adjacent");
    }

    #[test]
    fn merge_cages_cells() {
        let a = Cage {
            cells: vec![(0, 0)],
            op: Operation::Given(1),
        };
        let b = Cage {
            cells: vec![(0, 1)],
            op: Operation::Given(2),
        };
        let merged = merge_cages(&a, &b, Operation::Add(3));
        assert_eq!(merged.cells, vec![(0, 0), (0, 1)]);
        assert_eq!(merged.op, Operation::Add(3));
    }

    #[test]
    fn conflict_graph_l_shape() {
        // Cells: [(0,0), (0,1), (1,0)]
        // (0,0)&(0,1): same row 0 → (0,1)
        // (0,0)&(1,0): same col 0 → (0,2)
        // (0,1)&(1,0): different row AND different col → NOT included
        let cage = Cage {
            cells: vec![(0, 0), (0, 1), (1, 0)],
            op: Operation::Add(3),
        };
        let graph = conflict_graph(&cage);
        assert_eq!(graph, vec![(0, 1), (0, 2)]);
    }

    #[test]
    fn conflict_graph_linear() {
        // 3-cell row: [(0,0),(0,1),(0,2)] — all pairs share row 0
        let cage = Cage {
            cells: vec![(0, 0), (0, 1), (0, 2)],
            op: Operation::Add(6),
        };
        let graph = conflict_graph(&cage);
        assert_eq!(graph, vec![(0, 1), (0, 2), (1, 2)]);
    }

    #[test]
    fn contiguous_l_shape_is_contiguous() {
        let cage = Cage {
            cells: vec![(0, 0), (0, 1), (1, 1)],
            op: Operation::Add(6),
        };
        assert!(is_cage_contiguous(&cage));
    }

    #[test]
    fn single_cell_cage_is_contiguous() {
        let cage = Cage {
            cells: vec![(2, 2)],
            op: Operation::Given(3),
        };
        assert!(is_cage_contiguous(&cage));
    }

    #[test]
    fn diagonal_cage_is_not_contiguous() {
        let cage = Cage {
            cells: vec![(0, 0), (1, 1)],
            op: Operation::Add(4),
        };
        assert!(!is_cage_contiguous(&cage));
    }

    #[test]
    fn gap_cage_is_not_contiguous() {
        let cage = Cage {
            cells: vec![(0, 0), (0, 2)],
            op: Operation::Add(4),
        };
        assert!(!is_cage_contiguous(&cage));
    }
}
