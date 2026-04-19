use std::fmt;

pub type Value = u8;
pub type Cell = (usize, usize);
pub type Tuple = Vec<Value>;

#[derive(Debug, Clone, PartialEq)]
pub struct LatinSquare {
    pub n: usize,
    pub grid: Vec<Vec<Value>>,
}

impl LatinSquare {
    pub fn get(&self, cell: Cell) -> Value {
        self.grid[cell.0][cell.1]
    }
}

impl fmt::Display for LatinSquare {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in &self.grid {
            let s: Vec<String> = row.iter().map(|v| v.to_string()).collect();
            writeln!(f, "{}", s.join(" "))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    Add(u32),
    Sub(u32),
    Mul(u32),
    Div(u32),
    Given(Value),
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operation::Add(t) => write!(f, "{}+", t),
            Operation::Sub(t) => write!(f, "{}-", t),
            Operation::Mul(t) => write!(f, "{}×", t),
            Operation::Div(t) => write!(f, "{}÷", t),
            Operation::Given(v) => write!(f, "{}", v),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cage {
    pub cells: Vec<Cell>,
    pub op: Operation,
}

impl fmt::Display for Cage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {:?}", self.op, self.cells)
    }
}

#[derive(Debug, Clone)]
pub struct Puzzle {
    pub latin_square: LatinSquare,
    pub cages: Vec<Cage>,
}

impl Puzzle {
    /// Returns true if cages exactly partition all n² cells with no overlaps or gaps.
    pub fn validate(&self) -> bool {
        let n = self.latin_square.n;
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

impl fmt::Display for Puzzle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}x{} KenKen ({} cages)",
            self.latin_square.n,
            self.latin_square.n,
            self.cages.len()
        )?;
        for cage in &self.cages {
            writeln!(f, "  {}", cage)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_3x3_latin_square() -> LatinSquare {
        LatinSquare {
            n: 3,
            grid: vec![vec![2, 1, 3], vec![3, 2, 1], vec![1, 3, 2]],
        }
    }

    fn make_3x3_puzzle() -> Puzzle {
        let ls = make_3x3_latin_square();
        Puzzle {
            cages: vec![
                Cage {
                    cells: vec![(0, 0), (1, 0)],
                    op: Operation::Add(5),
                },
                Cage {
                    cells: vec![(0, 1), (0, 2)],
                    op: Operation::Add(4),
                },
                Cage {
                    cells: vec![(1, 1)],
                    op: Operation::Given(2),
                },
                Cage {
                    cells: vec![(1, 2), (2, 2)],
                    op: Operation::Mul(2),
                },
                Cage {
                    cells: vec![(2, 0), (2, 1)],
                    op: Operation::Sub(2),
                },
            ],
            latin_square: ls,
        }
    }

    #[test]
    fn latin_square_get() {
        let ls = make_3x3_latin_square();
        assert_eq!(ls.get((0, 0)), 2);
        assert_eq!(ls.get((1, 2)), 1);
        assert_eq!(ls.get((2, 1)), 3);
    }

    #[test]
    fn puzzle_validate_valid() {
        assert!(make_3x3_puzzle().validate());
    }

    #[test]
    fn puzzle_validate_duplicate_cell() {
        let mut puzzle = make_3x3_puzzle();
        puzzle.cages.push(Cage {
            cells: vec![(0, 0)],
            op: Operation::Given(2),
        });
        assert!(!puzzle.validate());
    }

    #[test]
    fn puzzle_validate_missing_cell() {
        let mut puzzle = make_3x3_puzzle();
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
