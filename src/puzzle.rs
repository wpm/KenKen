//! A [`Puzzle`] pairs a candidate grid with a set of [`Cage`] constraints
//! and all-different constraints for every row and column.

use std::collections::BTreeSet;

use rand::Rng;

use crate::{
    Cage, CageSlot, Cell, Cover, Error,
    Error::{
        CageConflict, DuplicateSlotPolyomino, InfeasibleOperation, RegionConflict, SlotNotInPuzzle,
    },
    Fill, Grid, Polyomino,
    constraints::{Constraint, all_different::AllDifferent, cage::operation::Operation},
    generator::generate::{SizeDistribution, default_op_policy, generate, generate_with},
    solver::solve::{Solver, State},
};

/// A KenKen puzzle: a grid of candidate values together with cage and
/// all-different constraints.
///
/// ## Fixpoint invariant
///
/// A *fixpoint* is a state in which applying all constraints produces no
/// further change — every cell's candidate set is already as narrow as the
/// constraints require. Every `Puzzle` upholds this invariant: construction
/// and mutation methods ([`with_cages`](Puzzle::with_cages),
/// [`new`](Puzzle::new), [`insert`](Puzzle::insert),
/// [`insert_region`](Puzzle::insert_region), [`promote`](Puzzle::promote),
/// [`demote`](Puzzle::demote)) propagate constraints to fixpoint before
/// returning. If propagation would empty any cell's candidate set (a
/// contradiction), the method returns `None` instead of a `Puzzle`, so a
/// `Puzzle` value always represents a consistent, fully propagated state.
/// Regions contribute no propagation, so `insert_region` skips that step.
#[derive(Debug, Clone)]
pub struct Puzzle {
    grid: Grid,
    all_different: Vec<AllDifferent>,
    slots: BTreeSet<CageSlot>,
}

impl Puzzle {
    /// Creates an `n`×`n` puzzle with no cages and all cells with total fills.
    ///
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`.
    pub fn new(n: usize) -> Result<Self, Error> {
        let grid = Grid::new(n)?;
        Ok(Self {
            grid: grid.clone(),
            all_different: grid.all_different_constraints()?,
            slots: BTreeSet::default(),
        })
    }

    /// Creates an `n`×`n` puzzle from a set of cages, then propagates
    /// all constraints. Returns `None` if propagation finds a contradiction.
    ///
    /// Thin wrapper around [`Puzzle::with_slots`] for the common case of a
    /// fully-operated puzzle (no draft regions).
    ///
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`.
    /// Returns [`SlotNotInPuzzle`] if any cage contains a cell outside the grid.
    /// Returns [`DuplicateSlotPolyomino`] if two cages share a polyomino.
    pub fn with_cages(n: usize, cages: &[Cage]) -> Result<Option<Self>, Error> {
        let slots: Vec<CageSlot> = cages.iter().cloned().map(CageSlot::Cage).collect();
        Self::with_slots(n, &slots)
    }

    /// Creates an `n`×`n` puzzle from a set of [`CageSlot`]s (mixed
    /// [`CageSlot::Region`]s and [`CageSlot::Cage`]s), then propagates all
    /// constraints. Returns `None` if propagation finds a contradiction.
    ///
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`.
    /// Returns [`SlotNotInPuzzle`] if any slot covers a cell outside the grid.
    /// Returns [`DuplicateSlotPolyomino`] if two slots share the same polyomino:
    /// [`CageSlot::cmp`] keys on the polyomino alone, so distinct slots over the
    /// same polyomino would silently collide in the puzzle's slot set.
    pub fn with_slots(n: usize, slots: &[CageSlot]) -> Result<Option<Self>, Error> {
        let grid = Grid::new(n)?;
        for slot in slots {
            if slot.cells().any(|c| c.row >= n || c.column >= n) {
                return Err(SlotNotInPuzzle(slot.clone()));
            }
        }
        let mut seen = BTreeSet::new();
        for slot in slots {
            if !seen.insert(slot.polyomino()) {
                return Err(DuplicateSlotPolyomino(slot.polyomino().clone()));
            }
        }
        Self {
            grid: grid.clone(),
            all_different: grid.all_different_constraints()?,
            slots: slots.iter().cloned().collect(),
        }
        .propagate()
    }

    pub const fn n(&self) -> usize {
        self.grid.n()
    }

    /// Returns a new puzzle with `grid` substituted in place of the current grid,
    /// then propagates all constraints. Returns `None` if propagation finds a
    /// contradiction. Cages and all-different constraints are carried over unchanged.
    fn set_grid(&self, grid: Grid) -> Result<Option<Self>, Error> {
        Self {
            grid,
            all_different: self.all_different.clone(),
            slots: self.slots.clone(),
        }
        .propagate()
    }

    /// Returns a new [`Puzzle`] with `cage` added. The puzzle's grid will reflect the constraints
    /// imposed by the new cage. If this results in a contradiction, this function returns `None`.
    ///
    /// This is idempotent. Adding a cage identical to one already present returns the puzzle
    /// unchanged.
    ///
    /// # Errors
    /// Returns [`CageConflict`] if `cage` overlaps any cage already in
    /// the puzzle not identical to `cage`.
    pub fn insert(&self, cage: Cage) -> Result<Option<Self>, Error> {
        // CageSlot's Ord compares by polyomino only, so any slot whose polyomino
        // intersects the new cage's is a candidate for either idempotency or
        // conflict — distinguish by exact value equality.
        match self
            .slots
            .iter()
            .find(|s| s.polyomino().intersects(cage.polyomino()))
        {
            Some(CageSlot::Cage(existing)) if existing == &cage => {
                return Ok(Some(self.clone()));
            }
            Some(CageSlot::Cage(_) | CageSlot::Region(_)) => return Err(CageConflict(cage)),
            None => {}
        }
        let mut slots = self.slots.clone();
        let _ = slots.insert(CageSlot::Cage(cage));
        Self {
            grid: self.grid.clone(),
            all_different: self.all_different.clone(),
            slots,
        }
        .propagate()
    }

    /// Returns a new puzzle with `cage` removed and constraints re-propagated.
    ///
    /// The entire grid is reset to `Fill::full(n)` before re-propagating with the remaining
    /// cages, so all constraints determine their new domains from scratch.
    ///
    /// This is idempotent. Attempting to remove a `cage` that is not present returns
    /// the puzzle unchanged.
    ///
    /// # Errors
    /// Returns [`Error`] if propagation encounters a cell outside the grid bounds.
    pub fn remove(&self, cage: &Cage) -> Result<Self, Error> {
        // CageSlot::Ord matches on polyomino alone; require exact value equality
        // before removing so a different-operation cage on the same polyomino
        // stays a no-op.
        let key = CageSlot::Cage(cage.clone());
        if self.slots.get(&key) != Some(&key) {
            return Ok(self.clone());
        }
        let mut slots = self.slots.clone();
        let _ = slots.remove(&key);
        let n = self.grid.n();
        Ok(Self {
            grid: Grid::new(n)?,
            all_different: self.all_different.clone(),
            slots,
        }
        .propagate()?
        // Safe: Grid::new(n) succeeds because n came from a valid Puzzle (n in 1..=9),
        // and filling every cell to full before propagating can only widen domains, never empty
        // them.
        .unwrap_or_else(|| unreachable!("widening fills cannot produce a contradiction")))
    }

    /// Returns a new [`Puzzle`] with `polyomino` added as a region — a claimed
    /// cage shape with no operation yet. Regions contribute no propagation, so
    /// the grid is unchanged.
    ///
    /// This is idempotent. Adding a region whose polyomino is value-identical
    /// to one already present returns the puzzle unchanged.
    ///
    /// # Errors
    /// Returns [`RegionConflict`] if `polyomino` overlaps any existing slot
    /// (cage or region) and is not value-identical to an existing region.
    pub fn insert_region(&self, polyomino: Polyomino) -> Result<Self, Error> {
        // CageSlot's Ord compares by polyomino only, so any slot whose
        // polyomino intersects this one is a candidate for either idempotency
        // (same region) or conflict — distinguish by exact value equality.
        match self
            .slots
            .iter()
            .find(|s| s.polyomino().intersects(&polyomino))
        {
            Some(CageSlot::Region(existing)) if existing == &polyomino => {
                return Ok(self.clone());
            }
            Some(CageSlot::Cage(_) | CageSlot::Region(_)) => {
                return Err(RegionConflict(polyomino));
            }
            None => {}
        }
        let mut slots = self.slots.clone();
        let _ = slots.insert(CageSlot::Region(polyomino));
        // Regions are excluded from apply_constraints, so no propagation
        // is needed.
        Ok(Self {
            grid: self.grid.clone(),
            all_different: self.all_different.clone(),
            slots,
        })
    }

    /// Returns a new [`Puzzle`] with the region at `polyomino` replaced by a
    /// cage carrying `op`, then re-propagated.
    ///
    /// This is idempotent on a same-op cage already present, and on a missing
    /// polyomino (no-op, returns the puzzle unchanged).
    ///
    /// # Errors
    /// - [`CageConflict`] if a cage with `polyomino` exists but uses a different operation.
    /// - [`InfeasibleOperation`] if `op` admits no valid tuples for `polyomino` (operator invalid
    ///   for the cell count, or target unreachable).
    pub fn promote(&self, polyomino: &Polyomino, op: Operation) -> Result<Option<Self>, Error> {
        // Grid::new validates 1..=9, so the conversion is total for any Puzzle.
        let n = u8::try_from(self.grid.n())
            .unwrap_or_else(|_| unreachable!("Puzzle invariant: n is in 1..=9"));
        // CageSlot::Ord matches by polyomino only, so the Region probe key
        // returns whatever slot variant lives at this polyomino.
        match self.slots.get(&CageSlot::Region(polyomino.clone())) {
            Some(CageSlot::Cage(existing)) if existing.operation() == op => {
                return Ok(Some(self.clone()));
            }
            Some(CageSlot::Cage(_)) => {
                return Err(CageConflict(Cage::new(n, polyomino.clone(), op)));
            }
            None => return Ok(Some(self.clone())),
            Some(CageSlot::Region(_)) => {}
        }
        let cage = Cage::new(n, polyomino.clone(), op);
        if cage.tuples().is_empty() {
            return Err(InfeasibleOperation(polyomino.clone(), op));
        }
        let mut slots = self.slots.clone();
        // BTreeSet::replace swaps the Region for the Cage in-place (Ord
        // matches by polyomino), preserving tab order.
        let _ = slots.replace(CageSlot::Cage(cage));
        Self {
            grid: self.grid.clone(),
            all_different: self.all_different.clone(),
            slots,
        }
        .propagate()
    }

    /// Returns a new [`Puzzle`] with the cage at `polyomino` replaced by a
    /// region, with the grid widened and constraints re-propagated.
    ///
    /// The grid is reset to `Fill::full(n)` before re-propagating, so all
    /// constraints determine their new domains from scratch.
    ///
    /// This is idempotent: if no cage at `polyomino` is present, the puzzle is
    /// returned unchanged.
    ///
    /// # Errors
    /// Returns [`Error`] if propagation encounters a cell outside the grid bounds.
    pub fn demote(&self, polyomino: &Polyomino) -> Result<Self, Error> {
        match self.slots.get(&CageSlot::Region(polyomino.clone())) {
            None | Some(CageSlot::Region(_)) => return Ok(self.clone()),
            Some(CageSlot::Cage(_)) => {}
        }
        let mut slots = self.slots.clone();
        let _ = slots.replace(CageSlot::Region(polyomino.clone()));
        let n = self.grid.n();
        Ok(Self {
            grid: Grid::new(n)?,
            all_different: self.all_different.clone(),
            slots,
        }
        .propagate()?
        .unwrap_or_else(|| unreachable!("widening fills cannot produce a contradiction")))
    }

    /// Returns the puzzle's cages in ascending order by polyomino cells
    /// (row-major). The puzzle invariant guarantees no two stored cages share
    /// a polyomino, so ties never arise.
    pub fn cages(&self) -> impl Iterator<Item = &Cage> {
        self.slots.iter().filter_map(CageSlot::as_cage)
    }

    /// Returns the puzzle's regions (claimed cage shapes with no operation
    /// yet) in ascending polyomino order.
    pub fn regions(&self) -> impl Iterator<Item = &Polyomino> {
        self.slots.iter().filter_map(CageSlot::as_region)
    }

    /// Returns all slots (cages and regions) in ascending polyomino order.
    pub fn slots(&self) -> impl Iterator<Item = &CageSlot> {
        self.slots.iter()
    }

    /// Returns each cell paired with its current candidate [`Fill`], in
    /// row-major order. Every `Puzzle` is propagated to a fixpoint, so each
    /// `Fill` is as narrow as the constraints require; for a solved puzzle,
    /// every `Fill` is a singleton.
    pub fn candidates(&self) -> impl Iterator<Item = (Cell, Fill)> {
        self.grid
            .cells()
            .map(|cell| (cell, self.grid.get(&cell).unwrap_or_default()))
    }

    /// Enumerates the puzzle's solutions via depth-first backtracking search.
    ///
    /// Each item is a fully solved [`Puzzle`] (every cell pinned to one value).
    /// The iterator is lazy: stop after the first item for uniqueness checks
    /// or drain it to count all solutions.
    pub fn solve(&self) -> impl Iterator<Item = Self> {
        Solver::new(self.clone())
    }

    /// Generates a random `n`×`n` puzzle using the default operation policy
    /// ([`Puzzle::default_op_policy`]) and the default cage-size distribution.
    ///
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`.
    pub fn generate<R: Rng>(n: usize, rng: &mut R) -> Result<Self, Error> {
        generate(n, rng)
    }

    /// Generates a random `n`×`n` puzzle with a caller-supplied operation
    /// policy and cage-size distribution.
    ///
    /// The pipeline samples a Latin square as the puzzle's solution, tiles
    /// the grid with random polyominos sized by `sizes`, then asks `op` to
    /// pick an [`Operation`] for each cage from its solved values.
    ///
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`, or any
    /// error returned by `op`.
    pub fn generate_with<R: Rng, F>(
        n: usize,
        rng: &mut R,
        op: F,
        sizes: SizeDistribution,
    ) -> Result<Self, Error>
    where
        F: Fn(&[u8], usize) -> Result<Operation, Error>,
    {
        generate_with(n, rng, op, sizes)
    }

    /// Default policy mapping a cage's solved values to an [`Operation`].
    ///
    /// - 1 cell: [`Operation::Given`].
    /// - 2 cells: [`Operation::Divide`] when divisible, otherwise [`Operation::Subtract`].
    /// - 3+ cells: [`Operation::Multiply`] when the product fits in `n²`, otherwise
    ///   [`Operation::Add`].
    ///
    /// # Errors
    /// Returns [`Error::EmptyOpPolicyValues`] if `values` is empty.
    pub fn default_op_policy(values: &[u8], n: usize) -> Result<Operation, Error> {
        default_op_policy(values, n)
    }

    #[cfg(test)]
    pub const fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Applies every constraint once, folding them left over the current grid.
    /// Returns `None` if any cell's fill becomes empty after applying constraints.
    ///
    /// Order is arbitrary: all constraints are monotone filters (they only
    /// remove candidates, never add them), so application order does not affect
    /// the fixed-point result.
    fn apply_constraints(&self) -> Result<Option<Grid>, Error> {
        self.all_different
            .iter()
            .map(|c| c as &dyn Constraint)
            .chain(self.cages().map(|c| c as &dyn Constraint))
            .try_fold(self.grid.clone(), |grid, c| c.apply_to(&grid))
            .map(|grid| (!grid.is_invalid()).then_some(grid))
    }
}

impl serde::Serialize for Puzzle {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Puzzle", 2)?;
        st.serialize_field("n", &self.grid.n())?;
        st.serialize_field("slots", &self.slots)?;
        st.end()
    }
}

impl<'de> serde::Deserialize<'de> for Puzzle {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct PuzzleData {
            n: usize,
            slots: Vec<CageSlot>,
        }
        let PuzzleData { n, slots } = PuzzleData::deserialize(d)?;
        Self::with_slots(n, &slots)
            .map_err(serde::de::Error::custom)?
            .ok_or_else(|| serde::de::Error::custom("puzzle slots produce a contradiction"))
    }
}

impl State for Puzzle {
    /// Applies all constraints repeatedly until the grid stabilizes or a
    /// contradiction is found.
    ///
    /// Returns `None` if any cell's fill becomes empty (indicating that no solution
    /// exists). Returns `Some(puzzle)` otherwise.
    ///
    /// # Errors
    /// Returns [`Error`] if a constraint references a cell outside the grid.
    fn propagate(&self) -> Result<Option<Self>, Error> {
        let mut puzzle = self.clone();
        loop {
            let Some(grid) = puzzle.apply_constraints()? else {
                return Ok(None);
            };
            if grid == puzzle.grid {
                break;
            }
            match puzzle.set_grid(grid)? {
                Some(p) => puzzle = p,
                None => return Ok(None),
            }
        }
        Ok(Some(puzzle))
    }

    /// Returns one child puzzle per candidate value of the most-constrained
    /// *unsolved* cell — the cell with the fewest remaining candidates among
    /// those with more than one candidate, breaking ties by cell order. Each
    /// child has that cell pinned to a single value.
    ///
    /// Returns an empty iterator when every cell is already a singleton.
    fn branch(&self) -> impl Iterator<Item = Self> {
        // Skip singletons: branching on a one-value fill would yield a child
        // identical to the parent, so the solver would loop on it forever.
        let pick = self
            .grid
            .cells()
            .map(|cell| (cell, self.grid.get(&cell).unwrap_or_default()))
            .filter(|(_, fill)| fill.len() > 1)
            .min_by(|(a, fa), (b, fb)| fa.len().cmp(&fb.len()).then_with(|| a.cmp(b)));
        pick.into_iter().flat_map(move |(cell, fill)| {
            let grid = self.grid.clone();
            fill.iter().filter_map(move |v| {
                self.set_grid(grid.clone().set(&cell, Fill::new([v])))
                    .ok()
                    .flatten()
            })
        })
    }

    fn is_solved(&self) -> bool {
        self.grid.is_solved()
    }
}

impl Cover for Puzzle {
    fn cells(&self) -> impl Iterator<Item = Cell> {
        self.grid.cells()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::Puzzle;
    use crate::{
        Cage, CageSlot, Cell, Cover, Error, Fill, Grid, Operation, Polyomino,
        constraints::test_utils::{cells, l_shape, pair, singleton},
        solver::solve::State,
    };

    fn puzzle_4() -> Puzzle {
        Puzzle::new(4).unwrap()
    }

    fn singleton_cage() -> Cage {
        Cage::new(4, singleton(), Operation::Given(3))
    }

    // --- Puzzle::with_cages ---

    #[test]
    fn with_cages_cage_outside_grid_returns_err() {
        // A 2-cell cage whose second cell lies outside the 1×1 grid.
        let out_of_bounds = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(0, 0), (0, 1)])).unwrap(),
            Operation::Add(3),
        );
        assert!(matches!(
            Puzzle::with_cages(1, &[out_of_bounds]),
            Err(Error::SlotNotInPuzzle(CageSlot::Cage(_)))
        ));
    }

    // --- Puzzle::with_slots ---

    #[test]
    fn with_slots_no_slots_returns_some() {
        let puzzle = Puzzle::with_slots(4, &[]).unwrap().unwrap();
        assert_eq!(puzzle.slots().count(), 0);
    }

    #[test]
    fn with_slots_mixed_regions_and_cages_succeeds() {
        // Region at (0,0), Cage with Given(2) at (1,1) — distinct polyominoes.
        let region = Polyomino::from_cells(&cells(&[(0, 0)])).unwrap();
        let cage = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(1, 1)])).unwrap(),
            Operation::Given(2),
        );
        let puzzle =
            Puzzle::with_slots(4, &[CageSlot::Region(region), CageSlot::Cage(cage.clone())])
                .unwrap()
                .unwrap();
        assert_eq!(puzzle.regions().count(), 1);
        itertools::assert_equal(puzzle.cages(), &[cage]);
        // The cage's Given(2) propagates and pins (1,1) to {2}.
        assert_eq!(puzzle.grid().get(&Cell::new(1, 1)).unwrap(), Fill::new([2]));
    }

    #[test]
    fn with_slots_region_outside_grid_returns_err() {
        // A region whose only cell sits outside a 2×2 grid.
        let region = Polyomino::from_cells(&cells(&[(5, 0)])).unwrap();
        assert!(matches!(
            Puzzle::with_slots(2, &[CageSlot::Region(region)]),
            Err(Error::SlotNotInPuzzle(CageSlot::Region(_)))
        ));
    }

    #[test]
    fn with_slots_cage_outside_grid_returns_err() {
        let cage = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(0, 0), (0, 1)])).unwrap(),
            Operation::Add(3),
        );
        assert!(matches!(
            Puzzle::with_slots(1, &[CageSlot::Cage(cage)]),
            Err(Error::SlotNotInPuzzle(CageSlot::Cage(_)))
        ));
    }

    #[test]
    fn with_slots_duplicate_polyomino_returns_err() {
        // A Region and a Cage over the same polyomino: CageSlot::Ord matches by
        // polyomino, so the BTreeSet would silently drop one. The constructor
        // must reject upfront.
        let cage = Cage::new(4, singleton(), Operation::Given(3));
        let slots = [CageSlot::Region(singleton()), CageSlot::Cage(cage)];
        assert!(matches!(
            Puzzle::with_slots(4, &slots),
            Err(Error::DuplicateSlotPolyomino(_))
        ));
    }

    #[test]
    fn with_slots_contradicting_cages_returns_none() {
        // Two Given(1) cages in the same row of a 2×2 grid is a contradiction.
        let c1 = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(0, 0)])).unwrap(),
            Operation::Given(1),
        );
        let c2 = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(0, 1)])).unwrap(),
            Operation::Given(1),
        );
        let slots = [CageSlot::Cage(c1), CageSlot::Cage(c2)];
        assert!(Puzzle::with_slots(2, &slots).unwrap().is_none());
    }

    #[test]
    fn new_invalid_size_returns_err() {
        assert!(Puzzle::new(0).is_err());
        assert!(Puzzle::new(10).is_err());
    }

    #[test]
    fn new_valid_size_succeeds() {
        assert!(Puzzle::new(1).is_ok());
        assert!(Puzzle::new(9).is_ok());
    }

    #[test]
    fn new_cells_have_full_candidates() {
        let puzzle = Puzzle::new(3).unwrap();
        assert!(
            puzzle
                .cells()
                .all(|c| puzzle.grid().get(&c).unwrap() == Fill::full(3))
        );
    }

    #[test]
    fn with_cages_no_cages_returns_some() {
        assert!(Puzzle::with_cages(4, &[]).unwrap().is_some());
    }

    #[test]
    fn with_cages_valid_cage_returns_some_and_propagates() {
        let cage = Cage::new(4, singleton(), Operation::Given(2));
        let puzzle = Puzzle::with_cages(4, &[cage]).unwrap().unwrap();
        assert_eq!(puzzle.grid().get(&Cell::new(0, 0)).unwrap(), Fill::new([2]));
    }

    #[test]
    fn with_cages_contradicting_cages_returns_none() {
        // Two Given(1) cages in the same row of a 2×2 grid: AllDifferent makes it unsolvable.
        let c1 = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(0, 0)])).unwrap(),
            Operation::Given(1),
        );
        let c2 = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(0, 1)])).unwrap(),
            Operation::Given(1),
        );
        assert!(Puzzle::with_cages(2, &[c1, c2]).unwrap().is_none());
    }

    #[test]
    fn with_cages_duplicate_polyomino_returns_err() {
        // Two cages over the same polyomino used to silently collapse in the
        // slot set (CageSlot::Ord matches by polyomino only); via with_slots
        // the constructor now rejects them.
        let c1 = Cage::new(4, singleton(), Operation::Given(3));
        let c2 = Cage::new(4, singleton(), Operation::Given(2));
        assert!(matches!(
            Puzzle::with_cages(4, &[c1, c2]),
            Err(Error::DuplicateSlotPolyomino(_))
        ));
    }

    #[test]
    fn n() {
        assert_eq!(puzzle_4().n(), 4);
    }

    // --- Puzzle::insert ---

    #[test]
    fn insert_non_overlapping_cage_succeeds() {
        let p = puzzle_4().insert(singleton_cage());
        assert!(p.is_ok());
    }

    #[test]
    fn insert_overlapping_cage_returns_err() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let overlap = Cage::new(4, singleton(), Operation::Given(1));
        assert!(matches!(p.insert(overlap), Err(Error::CageConflict(_))));
    }

    #[test]
    fn insert_is_non_destructive() {
        let base = puzzle_4();
        let _ = base.insert(singleton_cage()).unwrap();
        // base is unchanged — inserting into it again still succeeds
        assert!(base.insert(singleton_cage()).is_ok());
    }

    #[test]
    fn insert_duplicate_cage_is_idempotent() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let p2 = p.insert(singleton_cage()).unwrap().unwrap();
        assert_eq!(p.grid(), p2.grid());
    }

    // --- Puzzle::remove ---

    #[test]
    fn remove_present_cage_returns_puzzle_without_it() {
        let cage = singleton_cage();
        let p = puzzle_4().insert(cage.clone()).unwrap().unwrap();
        let p2 = p.remove(&cage).unwrap();
        // Can insert the same cage again after removal
        assert!(p2.insert(cage).is_ok());
    }

    #[test]
    fn remove_absent_cage_is_noop() {
        let p = puzzle_4();
        let p2 = p.remove(&singleton_cage()).unwrap();
        assert!(p2.insert(singleton_cage()).is_ok());
    }

    #[test]
    fn remove_same_polyomino_different_operation_is_noop() {
        // CageSlot::Ord matches on polyomino alone, so naive BTreeSet::remove
        // would drop the existing cage even when the operation differs.
        // remove() must guard with value equality and leave the puzzle intact.
        let original = singleton_cage(); // Given(3) at (0,0)
        let p = puzzle_4().insert(original.clone()).unwrap().unwrap();
        let other_op = Cage::new(4, singleton(), Operation::Given(2));
        let p2 = p.remove(&other_op).unwrap();
        // The original Given(3) cage is still enforced.
        assert_eq!(p2.grid().get(&Cell::new(0, 0)).unwrap(), Fill::new([3]));
        itertools::assert_equal(p2.cages(), &[original]);
    }

    #[test]
    fn remove_resets_cells_to_full() {
        // Insert Given(3) at (0,0), which pins that cell to {3}.
        // After removal, (0,0) should be widened back to the full candidate set.
        let cage = singleton_cage(); // Given(3) at (0,0)
        let p = puzzle_4().insert(cage.clone()).unwrap().unwrap();
        assert_eq!(p.grid().get(&Cell::new(0, 0)).unwrap(), Fill::new([3]));
        let p2 = p.remove(&cage).unwrap();
        assert_eq!(p2.grid().get(&Cell::new(0, 0)).unwrap(), Fill::full(4));
    }

    // --- Puzzle::insert_region ---

    #[test]
    fn insert_region_into_empty_puzzle_succeeds() {
        let p = puzzle_4().insert_region(singleton()).unwrap();
        assert_eq!(p.regions().count(), 1);
        assert_eq!(p.cages().count(), 0);
    }

    #[test]
    fn insert_region_does_not_narrow_cells() {
        // Regions contribute no propagation; the grid stays at Fill::full(n).
        let p = puzzle_4().insert_region(singleton()).unwrap();
        assert_eq!(p.grid().get(&Cell::new(0, 0)).unwrap(), Fill::full(4));
    }

    #[test]
    fn insert_region_overlap_with_cage_returns_err() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        assert!(matches!(
            p.insert_region(singleton()),
            Err(Error::RegionConflict(_))
        ));
    }

    #[test]
    fn insert_region_overlap_with_region_returns_err() {
        let p = puzzle_4().insert_region(singleton()).unwrap();
        // pair() = [(0,0),(0,1)] overlaps singleton() at (0,0).
        assert!(matches!(
            p.insert_region(pair()),
            Err(Error::RegionConflict(_))
        ));
    }

    #[test]
    fn insert_region_duplicate_is_idempotent() {
        let p = puzzle_4().insert_region(singleton()).unwrap();
        let p2 = p.insert_region(singleton()).unwrap();
        assert_eq!(p.regions().count(), p2.regions().count());
        assert_eq!(p.grid(), p2.grid());
    }

    #[test]
    fn insert_region_is_non_destructive() {
        let base = puzzle_4();
        let _ = base.insert_region(singleton()).unwrap();
        assert!(base.insert_region(singleton()).is_ok());
    }

    // --- Puzzle::promote ---

    #[test]
    fn promote_region_succeeds_and_propagates() {
        let p = puzzle_4().insert_region(singleton()).unwrap();
        let promoted = p
            .promote(&singleton(), Operation::Given(3))
            .unwrap()
            .unwrap();
        assert_eq!(promoted.cages().count(), 1);
        assert_eq!(promoted.regions().count(), 0);
        assert_eq!(
            promoted.grid().get(&Cell::new(0, 0)).unwrap(),
            Fill::new([3])
        );
    }

    #[test]
    fn promote_infeasible_target_returns_err() {
        // Add(1) on a 2-cell pair: minimum sum is 1 + 2 = 3, so no tuples.
        let p = puzzle_4().insert_region(pair()).unwrap();
        assert!(matches!(
            p.promote(&pair(), Operation::Add(1)),
            Err(Error::InfeasibleOperation(_, Operation::Add(1)))
        ));
    }

    #[test]
    fn promote_invalid_operator_for_shape_returns_err() {
        // Subtract on a 3-cell L-shape: operator only legal for 2-cell shapes.
        let p = puzzle_4().insert_region(l_shape()).unwrap();
        assert!(matches!(
            p.promote(&l_shape(), Operation::Subtract(1)),
            Err(Error::InfeasibleOperation(_, Operation::Subtract(1)))
        ));
    }

    #[test]
    fn promote_missing_polyomino_is_noop() {
        let p = puzzle_4();
        let p2 = p
            .promote(&singleton(), Operation::Given(3))
            .unwrap()
            .unwrap();
        assert_eq!(p2.cages().count(), 0);
        assert_eq!(p2.regions().count(), 0);
    }

    #[test]
    fn promote_existing_cage_same_op_is_idempotent() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let p2 = p
            .promote(&singleton(), Operation::Given(3))
            .unwrap()
            .unwrap();
        assert_eq!(p.grid(), p2.grid());
        itertools::assert_equal(p.cages(), p2.cages());
    }

    #[test]
    fn promote_existing_cage_different_op_returns_cage_conflict() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        assert!(matches!(
            p.promote(&singleton(), Operation::Given(2)),
            Err(Error::CageConflict(_))
        ));
    }

    // --- Puzzle::demote ---

    #[test]
    fn demote_widens_pinned_cell() {
        // Given(3) at (0,0) pins that cell to {3}; after demote, widens back.
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        assert_eq!(p.grid().get(&Cell::new(0, 0)).unwrap(), Fill::new([3]));
        let p2 = p.demote(&singleton()).unwrap();
        assert_eq!(p2.grid().get(&Cell::new(0, 0)).unwrap(), Fill::full(4));
    }

    #[test]
    fn demote_replaces_cage_with_region() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let p2 = p.demote(&singleton()).unwrap();
        assert_eq!(p2.cages().count(), 0);
        assert_eq!(p2.regions().count(), 1);
        assert_eq!(p2.regions().next().unwrap(), &singleton());
    }

    #[test]
    fn demote_absent_polyomino_is_noop() {
        let p = puzzle_4();
        let p2 = p.demote(&singleton()).unwrap();
        assert_eq!(p.grid(), p2.grid());
        assert_eq!(p2.slots().count(), 0);
    }

    #[test]
    fn demote_already_region_is_noop() {
        let p = puzzle_4().insert_region(singleton()).unwrap();
        let p2 = p.demote(&singleton()).unwrap();
        assert_eq!(p.grid(), p2.grid());
        assert_eq!(p2.regions().count(), 1);
    }

    #[test]
    fn demote_then_promote_round_trips() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let p2 = p
            .demote(&singleton())
            .unwrap()
            .promote(&singleton(), Operation::Given(3))
            .unwrap()
            .unwrap();
        assert_eq!(p.grid(), p2.grid());
        itertools::assert_equal(p.cages(), p2.cages());
    }

    // --- Puzzle::regions / Puzzle::slots ---

    #[test]
    fn regions_fresh_puzzle_is_empty() {
        assert_eq!(puzzle_4().regions().count(), 0);
    }

    #[test]
    fn regions_yields_only_region_variants() {
        // Cage at (0,0), region at (1,1) — disjoint polyominoes.
        let region_at_11 = Polyomino::from_cells(&cells(&[(1, 1)])).unwrap();
        let p = puzzle_4()
            .insert(singleton_cage())
            .unwrap()
            .unwrap()
            .insert_region(region_at_11.clone())
            .unwrap();
        itertools::assert_equal(p.regions(), &[region_at_11]);
    }

    #[test]
    fn slots_interleaves_regions_and_cages_by_polyomino_order() {
        let p = puzzle_4()
            .insert_region(Polyomino::from_cells(&cells(&[(1, 1)])).unwrap())
            .unwrap()
            .insert(singleton_cage())
            .unwrap()
            .unwrap();
        let collected: Vec<&CageSlot> = p.slots().collect();
        assert_eq!(collected.len(), 2);
        // (0,0) sorts before (1,1) in row-major order.
        assert!(matches!(collected[0], CageSlot::Cage(_)));
        assert!(matches!(collected[1], CageSlot::Region(_)));
    }

    #[test]
    fn slots_after_promote_preserves_order() {
        let before = puzzle_4()
            .insert_region(singleton())
            .unwrap()
            .insert_region(Polyomino::from_cells(&cells(&[(1, 1)])).unwrap())
            .unwrap();
        let after = before
            .promote(&singleton(), Operation::Given(3))
            .unwrap()
            .unwrap();
        let before_polys: Vec<&Polyomino> = before.slots().map(CageSlot::polyomino).collect();
        let after_polys: Vec<&Polyomino> = after.slots().map(CageSlot::polyomino).collect();
        assert_eq!(before_polys, after_polys);
    }

    // --- Puzzle::cages ---

    #[test]
    fn cages_fresh_puzzle_is_empty() {
        assert_eq!(puzzle_4().cages().count(), 0);
    }

    #[test]
    fn cages_yields_in_storage_order() {
        let a = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(0, 0)])).unwrap(),
            Operation::Given(3),
        );
        let b = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(1, 1)])).unwrap(),
            Operation::Given(2),
        );
        // Insert b first so storage order (BTreeSet by Cage::Ord) differs from insertion order.
        let puzzle = puzzle_4()
            .insert(b.clone())
            .unwrap()
            .unwrap()
            .insert(a.clone())
            .unwrap()
            .unwrap();
        itertools::assert_equal(puzzle.cages(), &[a, b]);
    }

    // --- Puzzle::candidates ---

    #[test]
    fn candidates_fresh_puzzle_yields_n_squared_pairs_with_full_fills() {
        let puzzle = puzzle_4();
        let pairs: Vec<(Cell, Fill)> = puzzle.candidates().collect();
        assert_eq!(pairs.len(), 16);
        assert!(pairs.iter().all(|(_, f)| *f == Fill::full(4)));
    }

    #[test]
    fn candidates_iterates_in_row_major_order() {
        let cells: Vec<Cell> = puzzle_4().candidates().map(|(c, _)| c).collect();
        let expected: Vec<Cell> = (0..4)
            .flat_map(|r| (0..4).map(move |c| Cell::new(r, c)))
            .collect();
        assert_eq!(cells, expected);
    }

    #[test]
    fn candidates_reflects_pinned_cell_after_given_cage() {
        let puzzle = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let (_, fill) = puzzle
            .candidates()
            .find(|(c, _)| *c == Cell::new(0, 0))
            .unwrap();
        assert_eq!(fill, Fill::new([3]));
    }

    // --- Puzzle::branch ---

    #[test]
    fn branch_fresh_4x4_yields_four_children() {
        // Fresh 4×4: every cell has fill {1,2,3,4}, so the most-constrained cell has 4 candidates.
        assert_eq!(puzzle_4().branch().count(), 4);
    }

    #[test]
    fn branch_children_each_propagate_without_error() {
        for child in puzzle_4().branch() {
            assert!(child.propagate().is_ok());
        }
    }

    #[test]
    fn branch_after_given_cage_skips_pinned_cell() {
        // Given(3) pins (0,0) to {3}; AllDifferent narrows the other
        // row-0 and column-0 cells to {1,2,4} (size 3). The most-constrained
        // unsolved cell is (0,1) (size 3, first by row-major tie-break), so
        // branch yields three children — one per remaining candidate.
        let puzzle = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        assert_eq!(puzzle.branch().count(), 3);
    }

    #[test]
    fn branch_children_pin_branching_cell_to_distinct_singleton_values() {
        // With (0,0)=3, branching happens on (0,1) ∈ {1,2,4}. Each child
        // pins (0,1) to a different value.
        let puzzle = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let mut values: Vec<Fill> = puzzle
            .branch()
            .map(|child| child.grid().get(&Cell::new(0, 1)).unwrap())
            .collect();
        assert!(values.iter().all(|f| f.len() == 1));
        values.sort_by_key(|f| f.iter().next().unwrap());
        values.dedup();
        assert_eq!(values.len(), puzzle.branch().count());
    }

    #[test]
    fn branch_solved_puzzle_yields_no_children() {
        // A 2×2 puzzle pinned by two non-overlapping Givens propagates to a
        // fully-singleton grid. branch() must yield no children so the solver
        // recognizes the state as a solution.
        let c1 = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(0, 0)])).unwrap(),
            Operation::Given(1),
        );
        let c2 = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(0, 1)])).unwrap(),
            Operation::Given(2),
        );
        let puzzle = Puzzle::new(2)
            .unwrap()
            .insert(c1)
            .unwrap()
            .unwrap()
            .insert(c2)
            .unwrap()
            .unwrap();
        assert_eq!(puzzle.branch().count(), 0);
    }

    // --- Puzzle::set_grid ---

    #[test]
    fn set_grid_replaces_grid_and_propagates() {
        // Pin (0,0) to {2} in a fresh grid and set it into a puzzle that has a
        // Given(2) cage at (0,0). Propagation should confirm the cell stays {2}.
        let cage = Cage::new(4, singleton(), Operation::Given(2));
        let puzzle = puzzle_4().insert(cage).unwrap().unwrap();
        let new_grid = puzzle.grid().clone().set(&Cell::new(0, 0), Fill::new([2]));
        let result = puzzle.set_grid(new_grid).unwrap().unwrap();
        assert_eq!(result.grid().get(&Cell::new(0, 0)).unwrap(), Fill::new([2]));
    }

    #[test]
    fn set_grid_preserves_cages() {
        // After set_grid, a cage that was present before is still enforced.
        let cage = singleton_cage(); // Given(3) at (0,0)
        let puzzle = puzzle_4().insert(cage).unwrap().unwrap();
        let same_grid = puzzle.grid().clone();
        let result = puzzle.set_grid(same_grid).unwrap().unwrap();
        assert_eq!(result.grid().get(&Cell::new(0, 0)).unwrap(), Fill::new([3]));
    }

    #[test]
    fn set_grid_returns_none_on_contradiction() {
        // Force an empty fill on a cell — propagation should detect it as invalid.
        let puzzle = puzzle_4();
        let contradicting_grid = puzzle.grid().clone().set(&Cell::new(0, 0), Fill::default());
        assert!(puzzle.set_grid(contradicting_grid).unwrap().is_none());
    }

    // --- Puzzle::propagate ---

    #[test]
    fn propagate_fresh_puzzle_returns_some() {
        assert!(puzzle_4().propagate().unwrap().is_some());
    }

    #[test]
    fn propagate_with_given_cage_narrows_cell() {
        let cage = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(0, 0)])).unwrap(),
            Operation::Given(2),
        );
        let puzzle = puzzle_4().insert(cage).unwrap().unwrap();
        let result = puzzle.propagate().unwrap().unwrap();
        assert_eq!(result.grid().get(&Cell::new(0, 0)).unwrap(), Fill::new([2]));
    }

    #[test]
    fn propagate_contradiction_returns_none() {
        // Two Given(1) cages in the same row forces two cells to {1},
        // which AllDifferent will eliminate to empty — a contradiction.
        // propagate() is called inside insert(), so insert() itself returns None.
        let c1 = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(0, 0)])).unwrap(),
            Operation::Given(1),
        );
        let c2 = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(0, 1)])).unwrap(),
            Operation::Given(1),
        );
        let result = Puzzle::new(2)
            .unwrap()
            .insert(c1)
            .unwrap()
            .unwrap()
            .insert(c2)
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn cover_cells_count_equals_n_squared() {
        assert_eq!(puzzle_4().cells().count(), 16);
    }

    #[test]
    fn propagate_is_idempotent() {
        let cage = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(0, 0)])).unwrap(),
            Operation::Given(2),
        );
        let puzzle = puzzle_4().insert(cage).unwrap().unwrap();
        let once = puzzle.propagate().unwrap().unwrap();
        let twice = once.propagate().unwrap().unwrap();
        assert_eq!(once.grid(), twice.grid());
    }

    // --- serde ---

    #[test]
    fn puzzle_serializes_to_json_with_n_and_slots() {
        let puzzle = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let json = serde_json::to_string(&puzzle).unwrap();
        assert!(json.contains("\"n\":4"));
        assert!(json.contains("\"slots\""));
    }

    #[test]
    fn puzzle_round_trips_through_json() {
        let original = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let restored: Puzzle = serde_json::from_str(&json).unwrap();
        assert_eq!(original.grid(), restored.grid());
        itertools::assert_equal(original.cages(), restored.cages());
    }

    #[test]
    fn puzzle_round_trips_with_no_cages() {
        let original = puzzle_4();
        let json = serde_json::to_string(&original).unwrap();
        let restored: Puzzle = serde_json::from_str(&json).unwrap();
        assert_eq!(original.grid(), restored.grid());
        itertools::assert_equal(original.cages(), restored.cages());
    }

    #[test]
    fn puzzle_round_trips_with_regions_preserved() {
        // Mixed Region + Cage slots must survive a JSON round-trip.
        let region_poly = Polyomino::from_cells(&cells(&[(0, 0)])).unwrap();
        let cage = Cage::new(
            4,
            Polyomino::from_cells(&cells(&[(1, 1)])).unwrap(),
            Operation::Given(2),
        );
        let grid = Grid::new(4).unwrap();
        let all_different = grid.all_different_constraints().unwrap();
        let original = Puzzle {
            grid,
            all_different,
            slots: [CageSlot::Region(region_poly), CageSlot::Cage(cage)]
                .into_iter()
                .collect(),
        }
        .propagate()
        .unwrap()
        .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let restored: Puzzle = serde_json::from_str(&json).unwrap();

        assert_eq!(original.slots, restored.slots);
        assert_eq!(original.grid(), restored.grid());
    }

    #[test]
    fn puzzle_serialize_format_locks_shape() {
        // Lock in the wire format: any future drift breaks this test.
        let region_poly = Polyomino::from_cells(&cells(&[(0, 0)])).unwrap();
        let cage = Cage::new(
            2,
            Polyomino::from_cells(&cells(&[(1, 1)])).unwrap(),
            Operation::Given(2),
        );
        let grid = Grid::new(2).unwrap();
        let all_different = grid.all_different_constraints().unwrap();
        // Skip propagate(): we only inspect the serialized shape (n + slots), not the grid.
        let puzzle = Puzzle {
            grid,
            all_different,
            slots: [CageSlot::Region(region_poly), CageSlot::Cage(cage)]
                .into_iter()
                .collect(),
        };
        assert_eq!(
            serde_json::to_value(&puzzle).unwrap(),
            serde_json::json!({
                "n": 2,
                "slots": [
                    {"Region": [{"row": 0, "column": 0}]},
                    {"Cage": {
                        "polyomino": [{"row": 1, "column": 1}],
                        "operation": {"Given": 2},
                        "n": 2,
                    }},
                ],
            }),
        );
    }

    #[test]
    fn puzzle_deserialize_contradicting_json_returns_err() {
        // Two Given(1) cages in the same row of a 2×2 grid is a contradiction.
        let json = r#"{"n":2,"slots":[
            {"Cage":{"polyomino":[{"row":0,"column":0}],"operation":{"Given":1},"n":2}},
            {"Cage":{"polyomino":[{"row":0,"column":1}],"operation":{"Given":1},"n":2}}
        ]}"#;
        assert!(serde_json::from_str::<Puzzle>(json).is_err());
    }

    #[test]
    fn puzzle_deserialize_invalid_n_returns_err() {
        let json = r#"{"n":99,"slots":[]}"#;
        assert!(serde_json::from_str::<Puzzle>(json).is_err());
    }

    #[test]
    fn puzzle_deserialize_out_of_grid_slot_returns_err() {
        // A region whose cell sits outside a 2×2 grid must be rejected.
        let json = r#"{"n":2,"slots":[{"Region":[{"row":5,"column":0}]}]}"#;
        assert!(serde_json::from_str::<Puzzle>(json).is_err());
    }

    #[test]
    fn puzzle_deserialize_duplicate_polyomino_slots_returns_err() {
        // Two slots over the same polyomino would silently collide in the BTreeSet
        // because CageSlot::Ord compares polyominos only — the deserializer must reject.
        let json = r#"{"n":2,"slots":[
            {"Region":[{"row":0,"column":0}]},
            {"Cage":{"polyomino":[{"row":0,"column":0}],"operation":{"Given":1},"n":2}}
        ]}"#;
        assert!(serde_json::from_str::<Puzzle>(json).is_err());
    }
}
