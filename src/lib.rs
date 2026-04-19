pub mod geometry;
pub mod history;
pub mod latin_square;
pub mod operation;
pub mod types;

use rand::prelude::IndexedRandom;
use rand::{Rng, RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
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

/// Returns the first solution, or `None` if the puzzle is unsolvable.
pub fn solve(puzzle: &Puzzle) -> Option<Vec<Vec<u32>>> {
    let mut grid = vec![vec![0u32; puzzle.size]; puzzle.size];
    backtrack_first(puzzle, &mut grid, 0, 0).then_some(grid)
}

/// Returns all solutions.
pub fn solve_all(puzzle: &Puzzle) -> Vec<Vec<Vec<u32>>> {
    let mut grid = vec![vec![0u32; puzzle.size]; puzzle.size];
    let mut solutions = Vec::new();
    backtrack_all(puzzle, &mut grid, 0, 0, &mut solutions);
    solutions
}

/// Returns true iff the puzzle has exactly one solution. Stops searching as
/// soon as a second solution is found, so it is much faster than
/// `solve_all(p).len() == 1` for ambiguous puzzles.
pub fn has_unique_solution(puzzle: &Puzzle) -> bool {
    count_solutions_up_to(puzzle, 2) == 1
}

/// Counts the puzzle's solutions, stopping once `limit` have been found.
pub fn count_solutions_up_to(puzzle: &Puzzle, limit: usize) -> usize {
    let mut grid = vec![vec![0u32; puzzle.size]; puzzle.size];
    let mut found = 0usize;
    backtrack_count(puzzle, &mut grid, 0, 0, limit, &mut found);
    found
}

fn next_cell(col: usize, row: usize, size: usize) -> (usize, usize) {
    if col + 1 == size {
        (row + 1, 0)
    } else {
        (row, col + 1)
    }
}

fn backtrack_first(puzzle: &Puzzle, grid: &mut Vec<Vec<u32>>, row: usize, col: usize) -> bool {
    if row == puzzle.size {
        return true;
    }
    let (next_row, next_col) = next_cell(col, row, puzzle.size);
    for val in 1..=(puzzle.size as u32) {
        if is_valid_placement(grid, puzzle.size, row, col, val) {
            grid[row][col] = val;
            if cages_satisfied(puzzle, grid, row, col)
                && backtrack_first(puzzle, grid, next_row, next_col)
            {
                return true;
            }
            grid[row][col] = 0;
        }
    }
    false
}

fn backtrack_all(
    puzzle: &Puzzle,
    grid: &mut Vec<Vec<u32>>,
    row: usize,
    col: usize,
    solutions: &mut Vec<Vec<Vec<u32>>>,
) {
    if row == puzzle.size {
        solutions.push(grid.clone());
        return;
    }
    let (next_row, next_col) = next_cell(col, row, puzzle.size);
    for val in 1..=(puzzle.size as u32) {
        if is_valid_placement(grid, puzzle.size, row, col, val) {
            grid[row][col] = val;
            if cages_satisfied(puzzle, grid, row, col) {
                backtrack_all(puzzle, grid, next_row, next_col, solutions);
            }
            grid[row][col] = 0;
        }
    }
}

fn backtrack_count(
    puzzle: &Puzzle,
    grid: &mut Vec<Vec<u32>>,
    row: usize,
    col: usize,
    limit: usize,
    found: &mut usize,
) {
    if *found >= limit {
        return;
    }
    if row == puzzle.size {
        *found += 1;
        return;
    }
    let (next_row, next_col) = next_cell(col, row, puzzle.size);
    for val in 1..=(puzzle.size as u32) {
        if is_valid_placement(grid, puzzle.size, row, col, val) {
            grid[row][col] = val;
            if cages_satisfied(puzzle, grid, row, col) {
                backtrack_count(puzzle, grid, next_row, next_col, limit, found);
                if *found >= limit {
                    return;
                }
            }
            grid[row][col] = 0;
        }
    }
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

/// Generate a random KenKen puzzle of side `size` that has a unique solution.
/// Returns `(puzzle, solution)` where `solution` is the intended unique
/// solution. Deterministic for a given seed.
pub fn generate(size: usize, seed: u64) -> (Puzzle, Vec<Vec<u32>>) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    generate_with_rng(size, &mut rng)
}

/// Generate a random unique-solution KenKen using a caller-provided RNG.
///
/// Uses a *coarsening* strategy: start from `size*size` singleton `Given` cages
/// (trivially unique), then repeatedly merge a random pair of adjacent cages
/// and assign an operation consistent with the target solution, keeping only
/// merges that preserve uniqueness.
pub fn generate_with_rng<R: RngCore>(size: usize, rng: &mut R) -> (Puzzle, Vec<Vec<u32>>) {
    assert!(size >= 2, "KenKen size must be at least 2");

    let solution = random_latin_square(size, rng);

    // Each cage carries a stable id so the blacklist survives index churn.
    let mut cages: Vec<Cage> = (0..size)
        .flat_map(|r| {
            let row = solution[r].clone();
            (0..size).map(move |c| Cage {
                cells: vec![(r, c)],
                op: Op::Given(row[c]),
            })
        })
        .collect();
    let mut ids: Vec<usize> = (0..cages.len()).collect();
    let mut next_id: usize = cages.len();
    let mut blacklist: HashSet<(usize, usize)> = HashSet::new();

    let target = ((size * size) / 3).max(2);
    let max_cage_size = (size - 1).max(3);

    loop {
        if cages.len() <= target {
            break;
        }
        let candidates = adjacent_cage_pairs(&cages, size, max_cage_size, &ids, &blacklist);
        if candidates.is_empty() {
            break;
        }
        let &(i, j) = candidates.choose(rng).unwrap();
        let pair_key = canonical_pair(ids[i], ids[j]);

        let mut merged_cells = cages[i].cells.clone();
        merged_cells.extend_from_slice(&cages[j].cells);
        let values: Vec<u32> = merged_cells.iter().map(|&(r, c)| solution[r][c]).collect();
        let op = choose_op_for_values(&values, rng);

        // Build a trial puzzle with i,j replaced by the merged cage.
        let mut trial_cages = Vec::with_capacity(cages.len() - 1);
        let mut trial_ids = Vec::with_capacity(ids.len() - 1);
        for (idx, cage) in cages.iter().enumerate() {
            if idx == i || idx == j {
                continue;
            }
            trial_cages.push(cage.clone());
            trial_ids.push(ids[idx]);
        }
        trial_cages.push(Cage {
            cells: merged_cells,
            op,
        });
        trial_ids.push(next_id);

        let trial = Puzzle {
            size,
            cages: trial_cages,
        };
        if has_unique_solution(&trial) {
            cages = trial.cages;
            ids = trial_ids;
            next_id += 1;
        } else {
            blacklist.insert(pair_key);
        }
    }

    (Puzzle { size, cages }, solution)
}

fn canonical_pair(a: usize, b: usize) -> (usize, usize) {
    if a <= b { (a, b) } else { (b, a) }
}

fn adjacent_cage_pairs(
    cages: &[Cage],
    size: usize,
    max_cage_size: usize,
    ids: &[usize],
    blacklist: &HashSet<(usize, usize)>,
) -> Vec<(usize, usize)> {
    // Map each cell to its owning cage index.
    let mut owner = vec![vec![usize::MAX; size]; size];
    for (idx, cage) in cages.iter().enumerate() {
        for &(r, c) in &cage.cells {
            owner[r][c] = idx;
        }
    }
    let mut pairs: HashSet<(usize, usize)> = HashSet::new();
    for (idx, cage) in cages.iter().enumerate() {
        for &(r, c) in &cage.cells {
            for (nr, nc) in neighbors(r, c) {
                if nr >= size || nc >= size {
                    continue;
                }
                let other = owner[nr][nc];
                if other == idx || other == usize::MAX {
                    continue;
                }
                let (a, b) = (idx.min(other), idx.max(other));
                if cages[a].cells.len() + cages[b].cells.len() > max_cage_size {
                    continue;
                }
                if blacklist.contains(&canonical_pair(ids[a], ids[b])) {
                    continue;
                }
                pairs.insert((a, b));
            }
        }
    }
    // HashSet iteration is randomized per-instance, so sort for determinism.
    let mut out: Vec<(usize, usize)> = pairs.into_iter().collect();
    out.sort_unstable();
    out
}

fn choose_op_for_values<R: RngCore>(values: &[u32], rng: &mut R) -> Op {
    match values.len() {
        0 => unreachable!("empty cage"),
        1 => Op::Given(values[0]),
        2 => {
            let (a, b) = (values[0], values[1]);
            let (big, small) = if a >= b { (a, b) } else { (b, a) };
            let mut choices: Vec<Op> = vec![Op::Add(a + b), Op::Sub(big - small), Op::Mul(a * b)];
            if small != 0 && big % small == 0 {
                choices.push(Op::Div(big / small));
            }
            let n = choices.len();
            choices.swap_remove(rng.random_range(0..n))
        }
        _ => {
            let sum: u32 = values.iter().sum();
            let prod: u32 = values.iter().product();
            if rng.random_bool(0.5) {
                Op::Add(sum)
            } else {
                Op::Mul(prod)
            }
        }
    }
}

fn random_latin_square<R: RngCore>(n: usize, rng: &mut R) -> Vec<Vec<u32>> {
    let ls = latin_square::generate_latin_square(n, rng);
    ls.grid
        .into_iter()
        .map(|row| row.into_iter().map(u32::from).collect())
        .collect()
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
    fn unique_puzzle_has_one_solution() {
        let puzzle = make_3x3_puzzle();
        let solutions = solve_all(&puzzle);
        assert_eq!(solutions.len(), 1);
        assert!(is_solution_valid(&puzzle, &solutions[0]));
    }

    #[test]
    fn solution_passes_validation() {
        let puzzle = make_3x3_puzzle();
        let solution = solve(&puzzle).unwrap();
        assert!(is_solution_valid(&puzzle, &solution));
    }

    // 3x3 puzzle with multiple solutions: one Add=6 cage per row.
    // Every valid 3x3 Latin square satisfies this (all 12 of them).
    //
    //  +---+---+---+
    //  | 6+        |
    //  +---+---+---+
    //  | 6+        |
    //  +---+---+---+
    //  | 6+        |
    //  +---+---+---+
    fn make_non_unique_puzzle() -> Puzzle {
        Puzzle {
            size: 3,
            cages: (0..3)
                .map(|r| Cage {
                    cells: (0..3).map(|c| (r, c)).collect(),
                    op: Op::Add(6),
                })
                .collect(),
        }
    }

    #[test]
    fn non_unique_puzzle_has_multiple_solutions() {
        let puzzle = make_non_unique_puzzle();
        let solutions = solve_all(&puzzle);
        // A 3x3 Latin square has exactly 12 valid arrangements.
        assert_eq!(solutions.len(), 12);
        for s in &solutions {
            assert!(is_solution_valid(&puzzle, s));
        }
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

    #[test]
    fn has_unique_solution_matches_solve_all() {
        let unique = make_3x3_puzzle();
        assert!(has_unique_solution(&unique));
        assert_eq!(count_solutions_up_to(&unique, 5), 1);

        let many = make_non_unique_puzzle();
        assert!(!has_unique_solution(&many));
        // Early-terminates at the 2nd solution even though 12 exist.
        assert_eq!(count_solutions_up_to(&many, 2), 2);
    }

    fn assert_generated_puzzle_is_good(size: usize, seed: u64) {
        let (puzzle, solution) = generate(size, seed);
        assert_eq!(puzzle.size, size);
        assert!(is_puzzle_covered(&puzzle), "cages must tile the grid");
        assert!(
            is_solution_valid(&puzzle, &solution),
            "claimed solution must satisfy the puzzle"
        );
        assert!(
            has_unique_solution(&puzzle),
            "generated puzzle must have a unique solution"
        );
        for cage in &puzzle.cages {
            assert!(is_cage_contiguous(cage), "every cage must be contiguous");
        }
    }

    #[test]
    fn generate_3x3_is_unique_and_valid() {
        assert_generated_puzzle_is_good(3, 42);
    }

    #[test]
    fn generate_4x4_is_unique_and_valid() {
        assert_generated_puzzle_is_good(4, 7);
    }

    #[test]
    fn generate_5x5_is_unique_and_valid() {
        assert_generated_puzzle_is_good(5, 2026);
    }

    #[test]
    fn generate_is_deterministic() {
        let a = generate(5, 123);
        let b = generate(5, 123);
        assert_eq!(a.1, b.1, "solutions should match");
        assert_eq!(a.0.cages.len(), b.0.cages.len(), "cage counts should match");
        for (ca, cb) in a.0.cages.iter().zip(b.0.cages.iter()) {
            assert_eq!(ca.cells, cb.cells);
            assert_eq!(ca.op, cb.op);
        }
    }

    #[test]
    fn generate_different_seeds_produce_different_puzzles() {
        let (_, sol1) = generate(5, 1);
        let (_, sol2) = generate(5, 2);
        assert_ne!(sol1, sol2);
    }

    #[test]
    fn generator_produces_nontrivial_cages() {
        // The generator should merge beyond all-singleton cages most of the time;
        // assert at least one cage of size >= 2 for a 5x5.
        let (puzzle, _) = generate(5, 99);
        assert!(
            puzzle.cages.iter().any(|c| c.cells.len() >= 2),
            "expected at least one multi-cell cage"
        );
    }
}
