#![allow(dead_code)]
use crate::constraints::{AllDifferent, Cage};
use crate::grid::Grid;
use crate::{Cell, Error, Index, Polyomino};
use std::collections::HashMap;

/// A `KenKen` puzzle is a `Grid` along with a set of `Constraint`s.
#[must_use]
#[derive(Debug, Clone)]
pub struct Puzzle {
    grid: Grid,
    rows: Vec<AllDifferent>,
    columns: Vec<AllDifferent>,
    /// Maps each cell to the polyomino of the cage that covers it.
    cage_cell: HashMap<Cell, Polyomino>,
    /// One entry per cage, keyed by its polyomino.
    cages: HashMap<Polyomino, Cage>,
}

impl Puzzle {
    /// # Errors
    /// Returns `Err` if `n` is not in `1..=9`.
    pub fn new(n: Index) -> Result<Self, Error> {
        Ok(Self {
            grid: Grid::new(n)?,
            rows: (0..n).map(|row| AllDifferent::row(n, row)).collect(),
            columns: (0..n)
                .map(|column| AllDifferent::column(n, column))
                .collect(),
            cage_cell: HashMap::new(),
            cages: HashMap::new(),
        })
    }

    #[must_use]
    pub const fn n(&self) -> Index {
        self.grid.n()
    }

    /// Returns a new puzzle with the cage inserted.
    ///
    /// Idempotent: if the exact same cage (by polyomino) is already present, returns a clone unchanged.
    /// # Errors
    /// Returns `Err` if any cell in the cage is already claimed by a *different* cage.
    pub fn insert_cage(mut self, cage: Cage) -> Result<Self, Error> {
        if self.cages.contains_key(cage.polyomino()) {
            return Ok(self);
        }
        let existing = cage.cells().into_iter().find_map(|cell| {
            self.cage_cell
                .get(&cell)
                .and_then(|poly| self.cages.get(poly))
        });
        if let Some(existing) = existing {
            return Err(Error::CageConflict(
                Box::new(cage),
                Box::new(existing.clone()),
            ));
        }
        let poly = cage.polyomino().clone();
        for cell in cage.cells() {
            self.cage_cell.insert(cell, poly.clone());
        }
        self.cages.insert(poly, cage);
        Ok(self)
    }

    /// Returns a new puzzle with the cage removed.
    ///
    /// Idempotent: if no such cage exists, returns unchanged.
    pub fn remove_cage(mut self, polyomino: &Polyomino) -> Self {
        if let Some(cage) = self.cages.remove(polyomino) {
            for cell in cage.cells() {
                self.cage_cell.remove(&cell);
            }
        }
        self
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::constraints::Operation;

    fn make_cage(cells: &[(usize, usize)], n: u8) -> Cage {
        let cells: Vec<Cell> = cells.iter().map(|&(r, c)| Cell::new(r, c)).collect();
        Cage::new(n, Polyomino::new(&cells), Operation::Add(n))
    }

    fn poly(cells: &[(usize, usize)]) -> Polyomino {
        Polyomino::new(
            &cells
                .iter()
                .map(|&(r, c)| Cell::new(r, c))
                .collect::<Vec<_>>(),
        )
    }

    // --- Puzzle::new ---

    #[test]
    fn new_returns_err_for_invalid_size() {
        assert!(Puzzle::new(0).is_err());
        assert!(Puzzle::new(10).is_err());
    }

    #[test]
    fn new_n_matches_size() {
        assert_eq!(Puzzle::new(4).unwrap().n(), 4);
    }

    #[test]
    fn new_has_no_cages() {
        let puzzle = Puzzle::new(4).unwrap();
        assert!(puzzle.cages.is_empty());
        assert!(puzzle.cage_cell.is_empty());
    }

    // --- Puzzle::insert_cage ---

    #[test]
    fn insert_cage_adds_cage_and_cells() {
        let cage = make_cage(&[(0, 0), (0, 1)], 4);
        let poly = cage.polyomino().clone();
        let puzzle = Puzzle::new(4).unwrap().insert_cage(cage).unwrap();
        assert!(puzzle.cages.contains_key(&poly));
        assert!(puzzle.cage_cell.contains_key(&Cell::new(0, 0)));
        assert!(puzzle.cage_cell.contains_key(&Cell::new(0, 1)));
    }

    #[test]
    fn insert_cage_cell_points_to_correct_polyomino() {
        let cage = make_cage(&[(0, 0), (0, 1)], 4);
        let poly = cage.polyomino().clone();
        let puzzle = Puzzle::new(4).unwrap().insert_cage(cage).unwrap();
        assert_eq!(puzzle.cage_cell[&Cell::new(0, 0)], poly);
        assert_eq!(puzzle.cage_cell[&Cell::new(0, 1)], poly);
    }

    #[test]
    fn insert_two_non_overlapping_cages() {
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(make_cage(&[(0, 0), (0, 1)], 4))
            .unwrap()
            .insert_cage(make_cage(&[(1, 0), (1, 1)], 4))
            .unwrap();
        assert_eq!(puzzle.cages.len(), 2);
        assert_eq!(puzzle.cage_cell.len(), 4);
    }

    #[test]
    fn insert_cage_idempotent() {
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(make_cage(&[(0, 0), (0, 1)], 4))
            .unwrap()
            .insert_cage(make_cage(&[(0, 0), (0, 1)], 4))
            .unwrap();
        assert_eq!(puzzle.cages.len(), 1);
        assert_eq!(puzzle.cage_cell.len(), 2);
    }

    #[test]
    fn insert_cage_conflict_returns_err() {
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(make_cage(&[(0, 0), (0, 1)], 4))
            .unwrap();
        let result = puzzle.insert_cage(make_cage(&[(0, 1), (0, 2)], 4));
        assert!(matches!(result, Err(Error::CageConflict(_, _))));
    }

    #[test]
    fn insert_cage_conflict_carries_new_and_existing_cage() {
        let existing = make_cage(&[(0, 0), (0, 1)], 4);
        let new = make_cage(&[(0, 1), (0, 2)], 4);
        let existing_poly = existing.polyomino().clone();
        let new_poly = new.polyomino().clone();
        let puzzle = Puzzle::new(4).unwrap().insert_cage(existing).unwrap();
        if let Err(Error::CageConflict(got_new, got_existing)) = puzzle.insert_cage(new) {
            assert_eq!(got_new.polyomino(), &new_poly);
            assert_eq!(got_existing.polyomino(), &existing_poly);
        } else {
            unreachable!("expected CageConflict");
        }
    }

    // --- Puzzle::remove_cage ---

    #[test]
    fn remove_cage_removes_cage_and_cells() {
        let cage = make_cage(&[(0, 0), (0, 1)], 4);
        let poly = cage.polyomino().clone();
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(cage)
            .unwrap()
            .remove_cage(&poly);
        assert!(!puzzle.cages.contains_key(&poly));
        assert!(!puzzle.cage_cell.contains_key(&Cell::new(0, 0)));
        assert!(!puzzle.cage_cell.contains_key(&Cell::new(0, 1)));
    }

    #[test]
    fn remove_cage_leaves_other_cages_intact() {
        let a = make_cage(&[(0, 0), (0, 1)], 4);
        let b = make_cage(&[(1, 0), (1, 1)], 4);
        let poly_a = a.polyomino().clone();
        let poly_b = b.polyomino().clone();
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(a)
            .unwrap()
            .insert_cage(b)
            .unwrap()
            .remove_cage(&poly_a);
        assert!(!puzzle.cages.contains_key(&poly_a));
        assert!(puzzle.cages.contains_key(&poly_b));
        assert!(puzzle.cage_cell.contains_key(&Cell::new(1, 0)));
    }

    #[test]
    fn remove_cage_idempotent() {
        let p = poly(&[(0, 0), (0, 1)]);
        let _ = Puzzle::new(4).unwrap().remove_cage(&p).remove_cage(&p);
    }

    #[test]
    fn remove_then_insert_succeeds() {
        let cage = make_cage(&[(0, 0), (0, 1)], 4);
        let poly = cage.polyomino().clone();
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(cage)
            .unwrap()
            .remove_cage(&poly)
            .insert_cage(make_cage(&[(0, 0), (0, 1)], 4))
            .unwrap();
        assert!(puzzle.cages.contains_key(&poly));
    }
}
