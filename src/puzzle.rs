use crate::state::State;
use std::cmp::Ordering;
use std::collections::BTreeMap;
pub(crate) use std::collections::BTreeSet;
use std::fmt;
use std::fmt::Display;

pub type Value = u8;
pub type Cell = (usize, usize);
pub type Tuple = Vec<Value>;

#[derive(Debug, PartialEq, Eq)]
pub enum PuzzleError {
    CellOutOfBounds(Cell),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    pub grid: Vec<Vec<Value>>,
}

impl Grid {
    /// # Panics
    ///
    /// Panics if any row length differs from the number of rows.
    #[must_use]
    pub fn new(grid: Vec<Vec<Value>>) -> Self {
        let n = grid.len();
        assert!(
            grid.iter().all(|row| row.len() == n),
            "grid must be square (n={n})"
        );
        Self { grid }
    }

    #[must_use]
    pub const fn n(&self) -> usize {
        self.grid.len()
    }

    #[must_use]
    pub fn get(&self, cell: Cell) -> Value {
        self.grid[cell.0][cell.1]
    }
}

impl Display for Grid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in &self.grid {
            let s: Vec<String> = row.iter().map(ToString::to_string).collect();
            writeln!(f, "{}", s.join(" "))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    Add(u32),
    Sub(u32),
    Mul(u32),
    Div(u32),
    Given(Value),
}

impl Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add(t) => write!(f, "{t}+"),
            Self::Sub(t) => write!(f, "{t}-"),
            Self::Mul(t) => write!(f, "{t}×"),
            Self::Div(t) => write!(f, "{t}÷"),
            Self::Given(v) => write!(f, "{v}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cage {
    pub op: Operation,
    pub cells: BTreeSet<Cell>,
}

impl PartialOrd for Cage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Cage {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cells
            .iter()
            .min()
            .cmp(&other.cells.iter().min())
            .then_with(|| self.cells.iter().cmp(other.cells.iter()))
    }
}

impl Display for Cage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:?}", self.op, self.cells)
    }
}

/// A `KenKen` puzzle: an n×n grid partitioned into cages, each with a solver state.
#[derive(Debug, Clone)]
pub struct Puzzle {
    pub n: usize,
    states: BTreeMap<Cage, State>,
}

impl Puzzle {
    /// Creates a new puzzle from a grid size and a set of cages.
    ///
    /// # Panics
    ///
    /// Panics if the cages do not exactly partition all n² cells.
    #[must_use]
    pub fn new(n: usize, cages: BTreeSet<Cage>) -> Self {
        assert!(
            cages_partition_grid(&cages, n),
            "cages must exactly partition all {n}×{n} cells"
        );
        let states = cages
            .into_iter()
            .map(|cage| {
                let state = State::new(&cage, n);
                (cage, state)
            })
            .collect();
        Self { n, states }
    }

    /// Returns the cage that contains `cell`.
    ///
    /// # Errors
    ///
    /// Returns `PuzzleError::CellOutOfBounds` if `cell` is not in any cage.
    pub fn cages_containing(&self, cell: Cell) -> Result<&Cage, PuzzleError> {
        self.states
            .keys()
            .find(|cage| cage.cells.contains(&cell))
            .ok_or(PuzzleError::CellOutOfBounds(cell))
    }

    /// Returns the state for the given cage.
    ///
    /// # Panics
    ///
    /// Panics if the cage is not part of this puzzle.
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn state(&self, cage: &Cage) -> &State {
        self.states.get(cage).expect("cage not found in puzzle")
    }
}

fn cages_partition_grid(cages: &BTreeSet<Cage>, n: usize) -> bool {
    let mut seen = vec![false; n * n];
    for cage in cages {
        for &(r, c) in &cage.cells {
            if r >= n || c >= n {
                return false;
            }
            let idx = r * n + c;
            if seen[idx] {
                return false;
            }
            seen[idx] = true;
        }
    }
    seen.iter().all(|&x| x)
}

impl Display for Puzzle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}x{} KenKen ({} cages)",
            self.n,
            self.n,
            self.states.len()
        )?;
        for cage in self.states.keys() {
            writeln!(f, "  {cage}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::fixtures::{make_3x3_latin_square, make_3x3_puzzle_cages};

    #[test]
    fn grid_new_square() {
        let g = Grid::new(vec![vec![1, 2], vec![3, 4]]);
        assert_eq!(g.n(), 2);
    }

    #[test]
    #[should_panic(expected = "grid must be square")]
    fn grid_new_non_square_panics() {
        let _ = Grid::new(vec![vec![1, 2, 3], vec![4, 5]]);
    }

    #[test]
    fn latin_square_get() {
        let ls = make_3x3_latin_square();
        assert_eq!(ls.get((0, 0)), 2);
        assert_eq!(ls.get((1, 2)), 1);
        assert_eq!(ls.get((2, 1)), 3);
    }

    #[test]
    fn latin_square_display() {
        assert_eq!(make_3x3_latin_square().to_string(), "2 1 3\n3 2 1\n1 3 2\n");
    }

    #[test]
    fn operation_display_all_variants() {
        assert_eq!(Operation::Add(6).to_string(), "6+");
        assert_eq!(Operation::Sub(2).to_string(), "2-");
        assert_eq!(Operation::Mul(12).to_string(), "12×");
        assert_eq!(Operation::Div(3).to_string(), "3÷");
        assert_eq!(Operation::Given(4).to_string(), "4");
    }

    #[test]
    fn cage_display() {
        let cage = Cage {
            op: Operation::Add(5),
            cells: BTreeSet::from([(0, 0), (1, 0)]),
        };
        let s = cage.to_string();
        assert!(s.contains("5+"));
        assert!(s.contains("(0, 0)"));
        assert!(s.contains("(1, 0)"));
    }

    #[test]
    fn cage_ordering_by_min_cell() {
        let top_left = Cage {
            op: Operation::Given(1),
            cells: BTreeSet::from([(0, 0)]),
        };
        let bottom_right = Cage {
            op: Operation::Given(2),
            cells: BTreeSet::from([(2, 2)]),
        };
        assert!(top_left < bottom_right);
    }

    #[test]
    fn cage_ordering_same_min_cell_differs_by_cells() {
        let small = Cage {
            op: Operation::Given(1),
            cells: BTreeSet::from([(0, 0)]),
        };
        let large = Cage {
            op: Operation::Given(1),
            cells: BTreeSet::from([(0, 0), (0, 1)]),
        };
        assert!(small < large);
    }

    #[test]
    fn puzzle_new_valid() {
        let p = Puzzle::new(3, make_3x3_puzzle_cages());
        assert_eq!(p.n, 3);
        assert_eq!(p.states.len(), 5);
    }

    #[test]
    #[should_panic(expected = "cages must exactly partition")]
    fn puzzle_new_duplicate_cell_panics() {
        let mut cages = make_3x3_puzzle_cages();
        cages.insert(Cage {
            op: Operation::Given(2),
            cells: BTreeSet::from([(0, 0)]),
        });
        let _ = Puzzle::new(3, cages);
    }

    #[test]
    #[should_panic(expected = "cages must exactly partition")]
    fn puzzle_new_missing_cell_panics() {
        let mut cages = make_3x3_puzzle_cages();
        cages.retain(|c| c.op != Operation::Sub(2));
        let _ = Puzzle::new(3, cages);
    }

    #[test]
    fn puzzle_display() {
        let p = Puzzle::new(3, make_3x3_puzzle_cages());
        let s = p.to_string();
        assert!(s.contains("3x3 KenKen"));
        assert!(s.contains("5 cages"));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn cages_containing_returns_cage_for_cell() {
        let p = Puzzle::new(3, make_3x3_puzzle_cages());
        assert_eq!(p.cages_containing((0, 0)).unwrap().op, Operation::Add(5));
        assert_eq!(p.cages_containing((0, 1)).unwrap().op, Operation::Add(4));
    }

    #[test]
    fn cages_containing_out_of_bounds_returns_error() {
        let p = Puzzle::new(3, make_3x3_puzzle_cages());
        assert_eq!(
            p.cages_containing((5, 5)),
            Err(PuzzleError::CellOutOfBounds((5, 5)))
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn puzzle_state_returns_state() {
        let p = Puzzle::new(3, make_3x3_puzzle_cages());
        let cage = p.states.keys().next().unwrap();
        let _state = p.state(cage);
    }
}
