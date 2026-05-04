#![allow(dead_code)]

use crate::shape::{Polyomino, is_connected_set};
use crate::types::{Cell, Error};
use rand::{Rng, RngExt};
use std::collections::{HashMap, HashSet};

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

    /// Moves `cell` from its current polyomino to `target`.
    ///
    /// # Errors
    /// - [`Error::CellNotCovered`] if `cell` is not in any polyomino.
    /// - [`Error::TargetNotAdjacent`] if `target` is not in this tiling, or has no cell
    ///   4-adjacent to `cell`.
    /// - [`Error::FlipWouldDisconnect`] if removing `cell` from its current polyomino would
    ///   leave that polyomino disconnected.
    pub fn flip(&mut self, cell: Cell, target: &Polyomino) -> Result<(), Error> {
        let source = self
            .find_cell(cell)
            .ok_or(Error::CellNotCovered(cell))?
            .clone();

        if &source == target {
            return Ok(());
        }

        if !self.contains(target) {
            return Err(Error::TargetNotAdjacent);
        }

        if !grid_neighbors(cell, self.n).any(|n| target.contains_cell(n)) {
            return Err(Error::TargetNotAdjacent);
        }

        let new_source_cells: HashSet<Cell> = source
            .as_slice()
            .iter()
            .copied()
            .filter(|c| *c != cell)
            .collect();
        if !is_connected_set(&new_source_cells) {
            return Err(Error::FlipWouldDisconnect(cell));
        }

        let mut new_target_cells: Vec<Cell> = target.as_slice().to_vec();
        new_target_cells.push(cell);

        self.polyominos.remove(&source);
        self.polyominos.remove(target);
        if !new_source_cells.is_empty() {
            let new_source: Vec<Cell> = new_source_cells.into_iter().collect();
            self.polyominos.insert(Polyomino::new(&new_source));
        }
        self.polyominos.insert(Polyomino::new(&new_target_cells));
        Ok(())
    }

    /// Replaces `p1` and `p2` in this tiling with the pair returned by
    /// [`random_merge_split`]; see that function for the splitting algorithm.
    ///
    /// # Errors
    /// [`Error::PolyominosNotAdjacent`] if either polyomino is not in this tiling, or
    /// any condition listed in [`random_merge_split`]'s errors holds.
    pub fn merge_split<R: Rng>(
        &mut self,
        p1: &Polyomino,
        p2: &Polyomino,
        rng: &mut R,
    ) -> Result<(), Error> {
        if !self.contains(p1) || !self.contains(p2) {
            return Err(Error::PolyominosNotAdjacent);
        }
        let (q1, q2) = random_merge_split(p1, p2, rng)?;
        self.polyominos.remove(p1);
        self.polyominos.remove(p2);
        self.polyominos.insert(q1);
        self.polyominos.insert(q2);
        Ok(())
    }

    /// Picks a random covered cell and a random 4-adjacent target cell, then flips.
    /// Returns true if the move was applied.
    pub fn random_flip<R: Rng>(&mut self, rng: &mut R) -> bool {
        let covered: Vec<Cell> = self
            .polyominos
            .iter()
            .flat_map(|p| p.as_slice().iter().copied())
            .collect();
        if covered.is_empty() {
            return false;
        }
        let cell = covered[rng.random_range(0..covered.len())];
        let neighbors: Vec<Cell> = grid_neighbors(cell, self.n).collect();
        if neighbors.is_empty() {
            return false;
        }
        let target_cell = neighbors[rng.random_range(0..neighbors.len())];
        let Some(target) = self.find_cell(target_cell).cloned() else {
            return false;
        };
        self.flip(cell, &target).is_ok()
    }

    /// Picks a random pair of adjacent polyominos and merge-splits them.
    /// Returns true if the move was applied.
    ///
    /// # Performance
    /// Enumerates all O(P²) polyomino pairs and tests adjacency for each, where `P` is the
    /// number of polyominos. For grids in the KenKen range (n ≤ 9, so P ≤ 81) this is
    /// ~6.5k adjacency checks per call; if used in a hot MCMC inner loop on larger grids,
    /// replace with a sampler that picks one cell uniformly and walks to a neighbor.
    pub fn random_merge_split<R: Rng>(&mut self, rng: &mut R) -> bool {
        if self.polyominos.len() < 2 {
            return false;
        }
        let polys: Vec<&Polyomino> = self.polyominos.iter().collect();
        let mut pair_indices: Vec<(usize, usize)> = Vec::new();
        for i in 0..polys.len() {
            for j in (i + 1)..polys.len() {
                if are_adjacent(polys[i], polys[j]) {
                    pair_indices.push((i, j));
                }
            }
        }
        if pair_indices.is_empty() {
            return false;
        }
        let (i, j) = pair_indices[rng.random_range(0..pair_indices.len())];
        let p1 = polys[i].clone();
        let p2 = polys[j].clone();
        self.merge_split(&p1, &p2, rng).is_ok()
    }
}

/// Merges two adjacent polyominos, builds a random spanning tree of the union,
/// removes a random edge, and returns the two resulting polyominos.
///
/// The spanning tree is built by randomized DFS, which does not sample uniformly from
/// the set of spanning trees of the merged region; the resulting split distribution is
/// biased toward those reachable by DFS. Use Wilson's algorithm if uniform sampling is
/// required.
///
/// # Errors
/// Returns [`Error::PolyominosNotAdjacent`] if `p1 == p2` or no cell of `p1` is
/// 4-adjacent to a cell of `p2`.
pub fn random_merge_split<R: Rng>(
    p1: &Polyomino,
    p2: &Polyomino,
    rng: &mut R,
) -> Result<(Polyomino, Polyomino), Error> {
    if p1 == p2 || !are_adjacent(p1, p2) {
        return Err(Error::PolyominosNotAdjacent);
    }

    let merged: Vec<Cell> = p1
        .as_slice()
        .iter()
        .chain(p2.as_slice().iter())
        .copied()
        .collect();

    let edges = random_spanning_tree(&merged, rng);
    let cut = rng.random_range(0..edges.len());
    let (a, _) = edges[cut];
    let component_a = tree_component(a, &edges, cut);
    let cells_a: Vec<Cell> = component_a.iter().copied().collect();
    let cells_b: Vec<Cell> = merged
        .iter()
        .copied()
        .filter(|c| !component_a.contains(c))
        .collect();

    Ok((Polyomino::new(&cells_a), Polyomino::new(&cells_b)))
}

/// In-bounds 4-neighbors of `cell` in an `n`×`n` grid.
fn grid_neighbors(cell: Cell, n: usize) -> impl Iterator<Item = Cell> {
    cell.neighbors_4()
        .filter(move |c| c.row < n && c.column < n)
}

/// Returns true if some cell of `a` is 4-adjacent to some cell of `b`.
fn are_adjacent(a: &Polyomino, b: &Polyomino) -> bool {
    a.as_slice()
        .iter()
        .flat_map(|c| c.neighbors_4())
        .any(|n| b.contains_cell(n))
}

/// Builds a random spanning tree of the subgraph induced by `cells` under 4-adjacency.
///
/// Uses randomized DFS: the start cell and neighbor selection are sampled from `rng`.
fn random_spanning_tree<R: Rng>(cells: &[Cell], rng: &mut R) -> Vec<(Cell, Cell)> {
    let cell_set: HashSet<Cell> = cells.iter().copied().collect();
    let mut visited: HashSet<Cell> = HashSet::with_capacity(cells.len());
    let mut edges: Vec<(Cell, Cell)> = Vec::with_capacity(cells.len().saturating_sub(1));
    let start = cells[rng.random_range(0..cells.len())];
    visited.insert(start);
    let mut stack: Vec<Cell> = vec![start];

    while let Some(&top) = stack.last() {
        let unvisited: Vec<Cell> = top
            .neighbors_4()
            .filter(|c| cell_set.contains(c) && !visited.contains(c))
            .collect();
        if unvisited.is_empty() {
            stack.pop();
            continue;
        }
        let next = unvisited[rng.random_range(0..unvisited.len())];
        edges.push((top, next));
        visited.insert(next);
        stack.push(next);
    }
    edges
}

/// Returns the connected component containing `start` in the tree given by `edges`,
/// excluding `edges[excluded_idx]`.
fn tree_component(start: Cell, edges: &[(Cell, Cell)], excluded_idx: usize) -> HashSet<Cell> {
    let mut adj: HashMap<Cell, Vec<Cell>> = HashMap::new();
    for (i, &(a, b)) in edges.iter().enumerate() {
        if i == excluded_idx {
            continue;
        }
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    }
    let mut visited: HashSet<Cell> = HashSet::new();
    visited.insert(start);
    let mut stack: Vec<Cell> = vec![start];
    while let Some(c) = stack.pop() {
        if let Some(neighbors) = adj.get(&c) {
            for &n in neighbors {
                if visited.insert(n) {
                    stack.push(n);
                }
            }
        }
    }
    visited
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::shape::is_connected;
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

    fn two_strip_tiling() -> Tiling {
        // 3x3 grid:
        //   A A A
        //   B B B
        //   C C C
        let mut t = Tiling::empty(3);
        t.insert(poly(&[(0, 0), (0, 1), (0, 2)]));
        t.insert(poly(&[(1, 0), (1, 1), (1, 2)]));
        t.insert(poly(&[(2, 0), (2, 1), (2, 2)]));
        t
    }

    #[test]
    fn flip_uncovered_cell_is_err() {
        let mut t = Tiling::empty(3);
        let target = poly(&[(0, 0)]);
        t.insert(target.clone());
        let r = t.flip(Cell::new(2, 2), &target);
        assert!(matches!(r, Err(Error::CellNotCovered(_))));
    }

    #[test]
    fn flip_non_adjacent_target_is_err() {
        let mut t = two_strip_tiling();
        let row_c = poly(&[(2, 0), (2, 1), (2, 2)]);
        // (0,0) is in row A; row C is not adjacent to (0,0).
        let r = t.flip(Cell::new(0, 0), &row_c);
        assert!(matches!(r, Err(Error::TargetNotAdjacent)));
    }

    #[test]
    fn flip_target_not_in_tiling_is_err() {
        let mut t = two_strip_tiling();
        // A polyomino that's adjacent to (0, 0) by cell coordinates but is not part of t.
        let stranger = poly(&[(0, 1)]);
        let r = t.flip(Cell::new(0, 0), &stranger);
        assert!(matches!(r, Err(Error::TargetNotAdjacent)));
    }

    #[test]
    fn flip_would_disconnect_is_err() {
        // Removing (0,1) from the row (0,0)-(0,1)-(0,2) splits it into two pieces.
        let mut t = Tiling::empty(3);
        t.insert(poly(&[(0, 0), (0, 1), (0, 2)]));
        let target = poly(&[(1, 1)]);
        t.insert(target.clone());
        let r = t.flip(Cell::new(0, 1), &target);
        assert!(matches!(r, Err(Error::FlipWouldDisconnect(_))));
    }

    #[test]
    fn flip_moves_cell_to_target() {
        let mut t = two_strip_tiling();
        let row_a = poly(&[(0, 0), (0, 1), (0, 2)]);
        // Move (1, 0) from row_b to row_a; remaining (1,1),(1,2) stays connected.
        t.flip(Cell::new(1, 0), &row_a).unwrap();
        let new_a = poly(&[(0, 0), (0, 1), (0, 2), (1, 0)]);
        let new_b = poly(&[(1, 1), (1, 2)]);
        assert!(t.contains(&new_a));
        assert!(t.contains(&new_b));
    }

    #[test]
    fn flip_emptying_source_removes_it() {
        let mut t = Tiling::empty(2);
        let solo = poly(&[(0, 0)]);
        let target = poly(&[(0, 1)]);
        t.insert(solo);
        t.insert(target.clone());
        t.flip(Cell::new(0, 0), &target).unwrap();
        assert_eq!(t.len(), 1);
        let new_target = poly(&[(0, 0), (0, 1)]);
        assert!(t.contains(&new_target));
    }

    #[test]
    fn merge_split_non_adjacent_is_err() {
        let mut t = two_strip_tiling();
        let row_a = poly(&[(0, 0), (0, 1), (0, 2)]);
        let row_c = poly(&[(2, 0), (2, 1), (2, 2)]);
        let mut r = rng();
        let res = t.merge_split(&row_a, &row_c, &mut r);
        assert!(matches!(res, Err(Error::PolyominosNotAdjacent)));
    }

    #[test]
    fn merge_split_same_polyomino_is_err() {
        let mut t = two_strip_tiling();
        let row_a = poly(&[(0, 0), (0, 1), (0, 2)]);
        let mut r = rng();
        let res = t.merge_split(&row_a, &row_a, &mut r);
        assert!(matches!(res, Err(Error::PolyominosNotAdjacent)));
    }

    #[test]
    fn merge_split_preserves_total_cells() {
        let mut t = two_strip_tiling();
        let row_a = poly(&[(0, 0), (0, 1), (0, 2)]);
        let row_b = poly(&[(1, 0), (1, 1), (1, 2)]);
        let mut r = rng();
        t.merge_split(&row_a, &row_b, &mut r).unwrap();
        let total: usize = t.polyominos().map(Polyomino::len).sum();
        assert_eq!(total, 9);
    }

    #[test]
    fn merge_split_with_singleton_polyomino() {
        // 2x2: a 1-cell polyomino at (0,0) plus the L-shaped polyomino covering the rest.
        // Merged: all 4 cells. Spanning tree has 3 edges; cutting any of them yields a
        // {1, 3} split (the only way to subdivide that doesn't recreate the original
        // singleton at (0,0) is to cut so that (0,0) ends up grouped with neighbors).
        let mut t = Tiling::empty(2);
        let single = poly(&[(0, 0)]);
        let l_shape = poly(&[(0, 1), (1, 0), (1, 1)]);
        t.insert(single.clone());
        t.insert(l_shape.clone());
        let mut r = rng();
        t.merge_split(&single, &l_shape, &mut r).unwrap();
        // Both inputs are gone; total cell count preserved; both new polys are connected.
        assert!(!t.contains(&single) || !t.contains(&l_shape));
        let total: usize = t.polyominos().map(Polyomino::len).sum();
        assert_eq!(total, 4);
        for p in t.polyominos() {
            assert!(!p.is_empty());
            assert!(is_connected(p.as_slice()));
        }
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn merge_split_yields_two_connected_components() {
        let mut t = two_strip_tiling();
        let row_a = poly(&[(0, 0), (0, 1), (0, 2)]);
        let row_b = poly(&[(1, 0), (1, 1), (1, 2)]);
        let mut r = rng();
        t.merge_split(&row_a, &row_b, &mut r).unwrap();
        // Row C should be untouched. The merged region produced two new polys that together
        // cover rows 0+1, both connected.
        let row_c = poly(&[(2, 0), (2, 1), (2, 2)]);
        assert!(t.contains(&row_c));
        let merged_cells: HashSet<Cell> = (0..2)
            .flat_map(|r| (0..3).map(move |c| Cell::new(r, c)))
            .collect();
        let mut covered: HashSet<Cell> = HashSet::new();
        let mut new_polys = 0;
        for p in t.polyominos() {
            if p == &row_c {
                continue;
            }
            new_polys += 1;
            assert!(is_connected(p.as_slice()));
            for c in p.as_slice() {
                covered.insert(*c);
            }
        }
        assert_eq!(new_polys, 2);
        assert_eq!(covered, merged_cells);
    }

    #[test]
    fn random_flip_preserves_invariants_smoke() {
        let mut r = rng();
        let mut t = Tiling::greedy(5, &SizeDistribution::Fixed(2), &mut r);
        let total_cells: usize = t.polyominos().map(Polyomino::len).sum();
        for _ in 0..20 {
            t.random_flip(&mut r);
        }
        let total_after: usize = t.polyominos().map(Polyomino::len).sum();
        assert_eq!(total_after, total_cells);
        for p in t.polyominos() {
            assert!(!p.is_empty());
            assert!(is_connected(p.as_slice()));
        }
    }

    #[test]
    fn random_merge_split_preserves_invariants_smoke() {
        let mut r = rng();
        let mut t = Tiling::greedy(5, &SizeDistribution::Fixed(2), &mut r);
        let total_cells: usize = t.polyominos().map(Polyomino::len).sum();
        for _ in 0..20 {
            t.random_merge_split(&mut r);
        }
        let total_after: usize = t.polyominos().map(Polyomino::len).sum();
        assert_eq!(total_after, total_cells);
        for p in t.polyominos() {
            assert!(!p.is_empty());
            assert!(is_connected(p.as_slice()));
        }
    }

    #[test]
    #[ignore = "slow: run with `cargo test -- --ignored`"]
    fn random_flip_preserves_invariants_fuzz() {
        for seed in 0..50 {
            let mut r = ChaCha8Rng::seed_from_u64(seed);
            let mut t = Tiling::greedy(9, &SizeDistribution::Uniform { min: 2, max: 4 }, &mut r);
            let total_cells: usize = t.polyominos().map(Polyomino::len).sum();
            for _ in 0..1000 {
                t.random_flip(&mut r);
            }
            let total_after: usize = t.polyominos().map(Polyomino::len).sum();
            assert_eq!(total_after, total_cells, "seed {seed}: total cells changed");
            for p in t.polyominos() {
                assert!(!p.is_empty(), "seed {seed}: empty polyomino");
                assert!(
                    is_connected(p.as_slice()),
                    "seed {seed}: disconnected polyomino"
                );
            }
        }
    }

    #[test]
    #[ignore = "slow: run with `cargo test -- --ignored`"]
    fn random_merge_split_preserves_invariants_fuzz() {
        for seed in 0..50 {
            let mut r = ChaCha8Rng::seed_from_u64(seed);
            let mut t = Tiling::greedy(9, &SizeDistribution::Uniform { min: 2, max: 4 }, &mut r);
            let total_cells: usize = t.polyominos().map(Polyomino::len).sum();
            for _ in 0..1000 {
                t.random_merge_split(&mut r);
            }
            let total_after: usize = t.polyominos().map(Polyomino::len).sum();
            assert_eq!(total_after, total_cells, "seed {seed}: total cells changed");
            for p in t.polyominos() {
                assert!(!p.is_empty(), "seed {seed}: empty polyomino");
                assert!(
                    is_connected(p.as_slice()),
                    "seed {seed}: disconnected polyomino"
                );
            }
        }
    }

    #[test]
    fn random_flip_returns_false_on_empty() {
        let mut r = rng();
        let mut t = Tiling::empty(3);
        assert!(!t.random_flip(&mut r));
    }

    #[test]
    fn random_merge_split_returns_false_with_fewer_than_two() {
        let mut r = rng();
        let mut t = Tiling::empty(3);
        assert!(!t.random_merge_split(&mut r));
        t.insert(poly(&[(0, 0)]));
        assert!(!t.random_merge_split(&mut r));
    }

    #[test]
    fn free_random_merge_split_preserves_union_of_cells() {
        let p1 = poly(&[(0, 0), (0, 1), (0, 2)]);
        let p2 = poly(&[(1, 0), (1, 1), (1, 2)]);
        let mut r = rng();
        let (q1, q2) = random_merge_split(&p1, &p2, &mut r).unwrap();
        let expected: HashSet<Cell> = p1
            .as_slice()
            .iter()
            .chain(p2.as_slice().iter())
            .copied()
            .collect();
        let got: HashSet<Cell> = q1
            .as_slice()
            .iter()
            .chain(q2.as_slice().iter())
            .copied()
            .collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn free_random_merge_split_returns_connected_polyominos() {
        let p1 = poly(&[(0, 0), (0, 1), (0, 2)]);
        let p2 = poly(&[(1, 0), (1, 1), (1, 2)]);
        let mut r = rng();
        let (q1, q2) = random_merge_split(&p1, &p2, &mut r).unwrap();
        assert!(!q1.is_empty());
        assert!(!q2.is_empty());
        assert!(is_connected(q1.as_slice()));
        assert!(is_connected(q2.as_slice()));
    }

    #[test]
    fn free_random_merge_split_same_polyomino_is_err() {
        let p = poly(&[(0, 0), (0, 1), (0, 2)]);
        let mut r = rng();
        let res = random_merge_split(&p, &p, &mut r);
        assert!(matches!(res, Err(Error::PolyominosNotAdjacent)));
    }

    #[test]
    fn free_random_merge_split_non_adjacent_is_err() {
        let p1 = poly(&[(0, 0), (0, 1), (0, 2)]);
        let p2 = poly(&[(2, 0), (2, 1), (2, 2)]);
        let mut r = rng();
        let res = random_merge_split(&p1, &p2, &mut r);
        assert!(matches!(res, Err(Error::PolyominosNotAdjacent)));
    }

    #[test]
    fn free_random_merge_split_is_deterministic_with_same_seed() {
        let p1 = poly(&[(0, 0), (0, 1), (0, 2)]);
        let p2 = poly(&[(1, 0), (1, 1), (1, 2)]);
        let mut r1 = ChaCha8Rng::seed_from_u64(123);
        let mut r2 = ChaCha8Rng::seed_from_u64(123);
        let (a1, a2) = random_merge_split(&p1, &p2, &mut r1).unwrap();
        let (b1, b2) = random_merge_split(&p1, &p2, &mut r2).unwrap();
        assert_eq!(a1, b1);
        assert_eq!(a2, b2);
    }
}
