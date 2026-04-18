use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Add(u32),
    Sub(u32),
    Mul(u32),
    Div(u32),
    Given(u32),
}

impl Op {
    fn is_satisfied(&self, filled: &[u32], all_filled: bool) -> bool {
        match self {
            Op::Given(t) => !all_filled || filled[0] == *t,
            Op::Add(t) => {
                let sum: u32 = filled.iter().sum();
                if all_filled { sum == *t } else { sum < *t }
            }
            Op::Mul(t) => {
                let prod: u32 = filled.iter().product();
                // Partial product equal to target is still satisfiable (multiply by 1s)
                if all_filled { prod == *t } else { prod <= *t }
            }
            Op::Sub(t) => !all_filled || filled[0].abs_diff(filled[1]) == *t,
            Op::Div(t) => {
                if !all_filled {
                    return true;
                }
                let (a, b) = (filled[0], filled[1]);
                let (big, small) = if a >= b { (a, b) } else { (b, a) };
                small != 0 && big % small == 0 && big / small == *t
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cage {
    pub cells: Vec<(usize, usize)>,
    pub op: Op,
}

pub struct Puzzle {
    pub size: usize,
    pub cages: Vec<Cage>,
}

pub fn is_cage_contiguous(cage: &Cage) -> bool {
    if cage.cells.len() <= 1 {
        return true;
    }
    let cell_set: HashSet<(usize, usize)> = cage.cells.iter().copied().collect();
    let mut visited = HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(cage.cells[0]);
    visited.insert(cage.cells[0]);
    while let Some((r, c)) = queue.pop_front() {
        for (nr, nc) in neighbors(r, c) {
            if cell_set.contains(&(nr, nc)) && visited.insert((nr, nc)) {
                queue.push_back((nr, nc));
            }
        }
    }
    visited.len() == cage.cells.len()
}

fn neighbors(r: usize, c: usize) -> [(usize, usize); 4] {
    // usize::MAX sentinels are filtered by is_cage_contiguous's cell_set lookup
    [
        (r.wrapping_sub(1), c),
        (r + 1, c),
        (r, c.wrapping_sub(1)),
        (r, c + 1),
    ]
}

/// Returns true if the cages exactly cover every cell in the puzzle grid with no overlaps.
pub fn is_puzzle_covered(puzzle: &Puzzle) -> bool {
    let mut seen = HashSet::new();
    for cage in &puzzle.cages {
        for &(r, c) in &cage.cells {
            if r >= puzzle.size || c >= puzzle.size {
                return false;
            }
            if !seen.insert((r, c)) {
                return false;
            }
        }
    }
    seen.len() == puzzle.size * puzzle.size
}

pub fn solve(puzzle: &Puzzle) -> Option<Vec<Vec<u32>>> {
    let mut grid = vec![vec![0u32; puzzle.size]; puzzle.size];
    if backtrack(puzzle, &mut grid, 0, 0) {
        Some(grid)
    } else {
        None
    }
}

fn backtrack(puzzle: &Puzzle, grid: &mut Vec<Vec<u32>>, row: usize, col: usize) -> bool {
    if row == puzzle.size {
        return true;
    }
    let (next_row, next_col) = if col + 1 == puzzle.size {
        (row + 1, 0)
    } else {
        (row, col + 1)
    };

    for val in 1..=(puzzle.size as u32) {
        if is_valid_placement(grid, puzzle.size, row, col, val) {
            grid[row][col] = val;
            if cages_satisfied(puzzle, grid, row, col)
                && backtrack(puzzle, grid, next_row, next_col)
            {
                return true;
            }
            grid[row][col] = 0;
        }
    }
    false
}

fn is_valid_placement(grid: &[Vec<u32>], size: usize, row: usize, col: usize, val: u32) -> bool {
    if grid[row].contains(&val) {
        return false;
    }
    if (0..size).any(|r| grid[r][col] == val) {
        return false;
    }
    true
}

fn cages_satisfied(puzzle: &Puzzle, grid: &[Vec<u32>], last_row: usize, last_col: usize) -> bool {
    for cage in &puzzle.cages {
        if !cage.cells.contains(&(last_row, last_col)) {
            continue;
        }
        let mut filled = Vec::with_capacity(cage.cells.len());
        for &(r, c) in &cage.cells {
            let v = grid[r][c];
            if v != 0 {
                filled.push(v);
            }
        }
        let all_filled = filled.len() == cage.cells.len();
        if !cage.op.is_satisfied(&filled, all_filled) {
            return false;
        }
    }
    true
}

pub fn is_solution_valid(puzzle: &Puzzle, grid: &[Vec<u32>]) -> bool {
    let size = puzzle.size;
    let expected: HashSet<u32> = (1..=(size as u32)).collect();
    for (i, row) in grid.iter().enumerate() {
        let row_set: HashSet<u32> = row.iter().copied().collect();
        if row_set != expected {
            return false;
        }
        let col: HashSet<u32> = (0..size).map(|r| grid[r][i]).collect();
        if col != expected {
            return false;
        }
    }
    for cage in &puzzle.cages {
        let values: Vec<u32> = cage.cells.iter().map(|&(r, c)| grid[r][c]).collect();
        if !cage.op.is_satisfied(&values, true) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    // 3x3 puzzle with unique solution [2,1,3 / 3,2,1 / 1,3,2]:
    //
    //  +-------+---+
    //  | 5+    | 4+|
    //  +   +---+---+
    //  |   | 2 | 2×|
    //  +---+---+   |
    //  | 2-|   |   |
    //  +---+---+---+
    //
    // Cages:
    //   (0,0),(1,0)  Add=5   2+3=5
    //   (0,1),(0,2)  Add=4   1+3=4
    //   (1,1)        Given=2
    //   (1,2),(2,2)  Mul=2   1*2=2
    //   (2,0),(2,1)  Sub=2   3-1=2
    fn make_3x3_puzzle() -> Puzzle {
        Puzzle {
            size: 3,
            cages: vec![
                Cage {
                    cells: vec![(0, 0), (1, 0)],
                    op: Op::Add(5),
                },
                Cage {
                    cells: vec![(0, 1), (0, 2)],
                    op: Op::Add(4),
                },
                Cage {
                    cells: vec![(1, 1)],
                    op: Op::Given(2),
                },
                Cage {
                    cells: vec![(1, 2), (2, 2)],
                    op: Op::Mul(2),
                },
                Cage {
                    cells: vec![(2, 0), (2, 1)],
                    op: Op::Sub(2),
                },
            ],
        }
    }

    #[test]
    fn solves_3x3_puzzle() {
        let puzzle = make_3x3_puzzle();
        let solution = solve(&puzzle).expect("puzzle should have a solution");

        let expected = vec![vec![2, 1, 3], vec![3, 2, 1], vec![1, 3, 2]];
        assert_eq!(solution, expected);
        assert!(is_solution_valid(&puzzle, &solution));
    }

    #[test]
    fn solution_passes_validation() {
        let puzzle = make_3x3_puzzle();
        let solution = solve(&puzzle).unwrap();
        assert!(is_solution_valid(&puzzle, &solution));
    }

    #[test]
    fn invalid_solution_fails_validation() {
        let puzzle = make_3x3_puzzle();
        let bad = vec![vec![2, 3, 1], vec![3, 2, 1], vec![1, 3, 2]];
        assert!(!is_solution_valid(&puzzle, &bad));
    }

    #[test]
    fn contiguous_cage_is_valid() {
        let cage = Cage {
            cells: vec![(0, 0), (0, 1), (1, 1)],
            op: Op::Add(6),
        };
        assert!(is_cage_contiguous(&cage));
    }

    #[test]
    fn single_cell_cage_is_contiguous() {
        let cage = Cage {
            cells: vec![(2, 2)],
            op: Op::Given(3),
        };
        assert!(is_cage_contiguous(&cage));
    }

    #[test]
    fn diagonal_cage_is_not_contiguous() {
        let cage = Cage {
            cells: vec![(0, 0), (1, 1)],
            op: Op::Add(4),
        };
        assert!(!is_cage_contiguous(&cage));
    }

    #[test]
    fn gap_cage_is_not_contiguous() {
        let cage = Cage {
            cells: vec![(0, 0), (0, 2)],
            op: Op::Add(4),
        };
        assert!(!is_cage_contiguous(&cage));
    }

    #[test]
    fn valid_puzzle_is_covered() {
        assert!(is_puzzle_covered(&make_3x3_puzzle()));
    }

    #[test]
    fn puzzle_with_missing_cell_is_not_covered() {
        let mut puzzle = make_3x3_puzzle();
        puzzle.cages.retain(|c| c.op != Op::Sub(2));
        assert!(!is_puzzle_covered(&puzzle));
    }

    #[test]
    fn puzzle_with_overlapping_cages_is_not_covered() {
        let mut puzzle = make_3x3_puzzle();
        puzzle.cages.push(Cage {
            cells: vec![(0, 0)],
            op: Op::Given(2),
        });
        assert!(!is_puzzle_covered(&puzzle));
    }

    #[test]
    fn puzzle_with_out_of_bounds_cell_is_not_covered() {
        let mut puzzle = make_3x3_puzzle();
        puzzle.cages.push(Cage {
            cells: vec![(5, 5)],
            op: Op::Given(1),
        });
        assert!(!is_puzzle_covered(&puzzle));
    }
}
