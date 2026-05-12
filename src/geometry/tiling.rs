#![allow(dead_code)]

use crate::geometry::shape::Polyomino;
use crate::types::Cell;
use rand::{Rng, RngExt};
use std::collections::HashSet;

/// Distribution over target cage sizes used by [`crate::generate_with`].
#[derive(Debug, Clone, Copy)]
pub enum SizeDistribution {
    /// Every polyomino has the same target size.
    Fixed(usize),
    /// Target size sampled uniformly from `min..=max`.
    Uniform { min: usize, max: usize },
}

impl SizeDistribution {
    fn sample<R: Rng>(self, rng: &mut R) -> usize {
        match self {
            Self::Fixed(s) => s,
            Self::Uniform { min, max } => rng.random_range(min..=max),
        }
    }
}

/// A set of disjoint, 4-connected [`Polyomino`]s on an `n`×`n` grid.
///
/// Invariants:
/// - No two polyominos share a cell.
/// - Every polyomino's cells are 4-connected.
/// - Every cell of every polyomino lies inside the `n`×`n` grid.
#[must_use]
#[derive(Debug, Clone)]
pub struct Tiling {
    n: usize,
    polyominos: HashSet<Polyomino>,
}

impl Tiling {
    /// Creates an empty tiling on an `n`×`n` grid.
    pub fn empty(n: usize) -> Self {
        Self {
            n,
            polyominos: HashSet::new(),
        }
    }

    #[must_use]
    pub const fn n(&self) -> usize {
        self.n
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.polyominos.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.polyominos.is_empty()
    }

    pub fn polyominos(&self) -> impl Iterator<Item = &Polyomino> {
        self.polyominos.iter()
    }

    /// Consumes the tiling, yielding owned polyominos. Use when the caller is the last owner
    /// and would otherwise have to clone each polyomino out of the borrowing iterator.
    pub fn into_polyominos(self) -> impl Iterator<Item = Polyomino> {
        self.polyominos.into_iter()
    }

    #[must_use]
    pub fn contains(&self, poly: &Polyomino) -> bool {
        self.polyominos.contains(poly)
    }

    /// Returns true if `cell` is covered by some polyomino in this tiling.
    #[must_use]
    pub fn covers(&self, cell: Cell) -> bool {
        self.find_cell(cell).is_some()
    }

    /// Returns true if every cell of the `n`×`n` grid is covered.
    #[must_use]
    pub fn covers_all(&self) -> bool {
        let total: usize = self.polyominos.iter().map(Polyomino::len).sum();
        total == self.n * self.n
    }

    /// Returns the polyomino containing `cell`, or `None` if `cell` is uncovered.
    #[must_use]
    pub fn find_cell(&self, cell: Cell) -> Option<&Polyomino> {
        self.polyominos.iter().find(|p| p.contains_cell(cell))
    }

    /// Inserts a polyomino without checking invariants.
    pub(crate) fn insert(&mut self, poly: Polyomino) {
        self.polyominos.insert(poly);
    }

    /// Removes a polyomino if present; returns true if it was present.
    pub(crate) fn remove(&mut self, poly: &Polyomino) -> bool {
        self.polyominos.remove(poly)
    }

    /// Builds a tiling that fully covers an `n`×`n` grid by greedy growth.
    ///
    /// Repeatedly seeds a random uncovered cell, grows it by absorbing random 4-adjacent
    /// uncovered cells until the target size sampled from `dist` is reached or no candidates
    /// remain, then starts a new polyomino.
    pub fn greedy<R: Rng>(n: usize, dist: &SizeDistribution, rng: &mut R) -> Self {
        let mut tiling = Self::empty(n);
        let mut covered: HashSet<Cell> = HashSet::with_capacity(n * n);

        while covered.len() < n * n {
            let uncovered: Vec<Cell> = (0..n)
                .flat_map(|r| (0..n).map(move |c| Cell::new(r, c)))
                .filter(|c| !covered.contains(c))
                .collect();
            let seed = uncovered[rng.random_range(0..uncovered.len())];
            let target_size = dist.sample(rng);

            let mut cells: HashSet<Cell> = HashSet::new();
            cells.insert(seed);
            // Frontier may contain duplicates; dedup happens on pop via the cells/covered checks.
            let mut frontier: Vec<Cell> = grid_neighbors(seed, n)
                .filter(|c| !covered.contains(c))
                .collect();

            while cells.len() < target_size && !frontier.is_empty() {
                let pick_idx = rng.random_range(0..frontier.len());
                let pick = frontier.swap_remove(pick_idx);
                if !cells.insert(pick) {
                    continue;
                }
                for neighbor in grid_neighbors(pick, n) {
                    if !covered.contains(&neighbor) && !cells.contains(&neighbor) {
                        frontier.push(neighbor);
                    }
                }
            }

            for c in &cells {
                covered.insert(*c);
            }
            let cells: Vec<Cell> = cells.into_iter().collect();
            tiling.polyominos.insert(Polyomino::new(&cells));
        }

        tiling
    }
}

/// In-bounds 4-neighbors of `cell` in an `n`×`n` grid.
fn grid_neighbors(cell: Cell, n: usize) -> impl Iterator<Item = Cell> {
    cell.neighbors_4()
        .filter(move |c| c.row < n && c.column < n)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::geometry::shape::is_connected;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn rng() -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(42)
    }

    fn poly(cells: &[(usize, usize)]) -> Polyomino {
        let cells: Vec<Cell> = cells.iter().map(|&(r, c)| Cell::new(r, c)).collect();
        Polyomino::new(&cells)
    }

    #[test]
    fn empty_tiling_has_no_polyominos() {
        let t = Tiling::empty(4);
        assert_eq!(t.n(), 4);
        assert_eq!(t.len(), 0);
        assert!(t.is_empty());
        assert!(!t.covers_all());
    }

    #[test]
    fn covers_finds_inserted_cell() {
        let mut t = Tiling::empty(3);
        t.insert(poly(&[(0, 0), (0, 1)]));
        assert!(t.covers(Cell::new(0, 0)));
        assert!(t.covers(Cell::new(0, 1)));
        assert!(!t.covers(Cell::new(1, 0)));
    }

    #[test]
    fn find_cell_returns_owning_polyomino() {
        let mut t = Tiling::empty(3);
        let p = poly(&[(0, 0), (0, 1)]);
        t.insert(p.clone());
        assert_eq!(t.find_cell(Cell::new(0, 0)), Some(&p));
        assert_eq!(t.find_cell(Cell::new(2, 2)), None);
    }

    #[test]
    fn covers_all_true_when_full() {
        let mut t = Tiling::empty(2);
        t.insert(poly(&[(0, 0), (0, 1), (1, 0), (1, 1)]));
        assert!(t.covers_all());
    }

    fn cells_of(t: &Tiling) -> Vec<Cell> {
        let mut cells: Vec<Cell> = t
            .polyominos()
            .flat_map(|p| p.as_slice().iter().copied())
            .collect();
        cells.sort();
        cells
    }

    #[test]
    fn greedy_covers_all_cells() {
        for n in [1, 2, 3, 4, 5, 9] {
            let mut r = rng();
            let t = Tiling::greedy(n, &SizeDistribution::Uniform { min: 1, max: 4 }, &mut r);
            assert!(t.covers_all(), "n={n} should cover all");
            let cells = cells_of(&t);
            assert_eq!(cells.len(), n * n);
        }
    }

    #[test]
    fn greedy_no_overlapping_cells() {
        for n in [1, 2, 3, 4, 5, 9] {
            let mut r = rng();
            let t = Tiling::greedy(n, &SizeDistribution::Uniform { min: 1, max: 4 }, &mut r);
            let mut seen: HashSet<Cell> = HashSet::new();
            for p in t.polyominos() {
                for c in p.as_slice() {
                    assert!(seen.insert(*c), "cell {c:?} appears in two polyominos");
                }
            }
        }
    }

    #[test]
    fn greedy_all_polyominos_connected() {
        let mut r = rng();
        let t = Tiling::greedy(9, &SizeDistribution::Uniform { min: 1, max: 4 }, &mut r);
        for p in t.polyominos() {
            assert!(is_connected(p.as_slice()));
        }
    }

    #[test]
    fn greedy_fixed_size_1_produces_n_squared_polyominos() {
        let mut r = rng();
        let t = Tiling::greedy(4, &SizeDistribution::Fixed(1), &mut r);
        assert_eq!(t.len(), 16);
        for p in t.polyominos() {
            assert_eq!(p.len(), 1);
        }
    }

    #[test]
    fn greedy_fixed_size_large_enough_produces_one_polyomino() {
        let mut r = rng();
        let t = Tiling::greedy(4, &SizeDistribution::Fixed(100), &mut r);
        assert_eq!(t.len(), 1);
        assert!(t.covers_all());
    }

    #[test]
    fn greedy_uniform_sizes_stay_in_range() {
        let mut r = rng();
        let t = Tiling::greedy(9, &SizeDistribution::Uniform { min: 1, max: 4 }, &mut r);
        for p in t.polyominos() {
            assert!(!p.is_empty() && p.len() <= 4);
        }
    }

    #[test]
    fn greedy_is_deterministic_with_same_seed() {
        let mut r1 = rng();
        let mut r2 = rng();
        let a = Tiling::greedy(5, &SizeDistribution::Uniform { min: 1, max: 4 }, &mut r1);
        let b = Tiling::greedy(5, &SizeDistribution::Uniform { min: 1, max: 4 }, &mut r2);
        let a_polys: HashSet<&Polyomino> = a.polyominos().collect();
        let b_polys: HashSet<&Polyomino> = b.polyominos().collect();
        assert_eq!(a_polys, b_polys);
    }

    #[test]
    fn greedy_differs_with_different_seeds() {
        let mut r1 = ChaCha8Rng::seed_from_u64(1);
        let mut r2 = ChaCha8Rng::seed_from_u64(2);
        let a = Tiling::greedy(5, &SizeDistribution::Uniform { min: 1, max: 4 }, &mut r1);
        let b = Tiling::greedy(5, &SizeDistribution::Uniform { min: 1, max: 4 }, &mut r2);
        let a_polys: HashSet<&Polyomino> = a.polyominos().collect();
        let b_polys: HashSet<&Polyomino> = b.polyominos().collect();
        assert_ne!(a_polys, b_polys);
    }
}
