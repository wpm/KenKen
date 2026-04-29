use std::cmp::Ordering;
pub(crate) use std::collections::BTreeSet;
use std::fmt;
use std::fmt::Display;

pub type Value = u8;
pub type Cell = (usize, usize);
pub type Tuple = Vec<Value>;

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

#[derive(Debug, Clone)]
pub struct Puzzle {
    pub latin_square: Grid,
    pub cages: BTreeSet<Cage>,
}

impl Puzzle {
    /// Returns true if cages exactly partition all n² cells with no overlaps or gaps.
    #[must_use]
    pub fn validate(&self) -> bool {
        let n = self.latin_square.n();
        let mut seen = std::collections::HashSet::new();
        for cage in &self.cages {
            for &cell in &cage.cells {
                if cell.0 >= n || cell.1 >= n {
                    return false;
                }
                if !seen.insert(cell) {
                    return false;
                }
            }
        }
        seen.len() == n * n
    }
}

impl Display for Puzzle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}x{} KenKen ({} cages)",
            self.latin_square.n(),
            self.latin_square.n(),
            self.cages.len()
        )?;
        for cage in &self.cages {
            writeln!(f, "  {cage}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::fixtures::{make_3x3_latin_square, make_3x3_unique_puzzle};

    #[test]
    fn latin_square_get() {
        let ls = make_3x3_latin_square();
        assert_eq!(ls.get((0, 0)), 2);
        assert_eq!(ls.get((1, 2)), 1);
        assert_eq!(ls.get((2, 1)), 3);
    }

    #[test]
    fn puzzle_validate_valid() {
        assert!(make_3x3_unique_puzzle().validate());
    }

    #[test]
    fn puzzle_validate_duplicate_cell() {
        let mut puzzle = make_3x3_unique_puzzle();
        puzzle.cages.insert(Cage {
            op: Operation::Given(2),
            cells: BTreeSet::from([(0, 0)]),
        });
        assert!(!puzzle.validate());
    }

    #[test]
    fn puzzle_validate_missing_cell() {
        let mut puzzle = make_3x3_unique_puzzle();
        puzzle.cages.retain(|c| c.op != Operation::Sub(2));
        assert!(!puzzle.validate());
    }

    #[test]
    fn operation_display() {
        assert_eq!(Operation::Add(5).to_string(), "5+");
        assert_eq!(Operation::Given(3).to_string(), "3");
    }

    #[test]
    fn latin_square_display() {
        let ls = make_3x3_latin_square();
        let s = ls.to_string();
        assert!(s.contains("2 1 3"));
    }
}
