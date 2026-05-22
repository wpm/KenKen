//! A [`Puzzle`] pairs a domain [`Store`] with a set of [`Cage`] constraints and
//! all-different constraints for every row and column.

use std::collections::BTreeSet;

use rand::Rng;

#[cfg(test)]
use crate::variable::Variable;
use crate::{
    Cage, CageSlot, Cell, Domain, Error,
    Error::{
        CageConflict, DuplicateSlotPolyomino, InfeasibleOperation, InvalidGridSize, RegionConflict,
        SlotNotInPuzzle,
    },
    Operation, Polyomino,
    all_different::AllDifferent,
    cache::Cache,
    constraint::{Constraint, Outcome, PropagationCtx, propagate_to_fixpoint},
    cover::Cover,
    generator::generate::{SizeDistribution, default_op_policy, generate, generate_with},
    solver::Search,
    store::Store,
};

/// The one homogeneous constraint type the engine propagates: a cage or an
/// all-different. [`propagate_to_fixpoint`] is generic over a single constraint
/// type, and a KenKen puzzle mixes the two, so this enum unifies them.
#[derive(Debug, Clone)]
enum KenKenConstraint {
    Cage(Cage),
    AllDiff(AllDifferent),
}

impl Constraint<Cell> for KenKenConstraint {
    fn propagate(&self, ctx: &mut PropagationCtx<Cell>) -> Outcome {
        match self {
            Self::Cage(c) => c.propagate(ctx),
            Self::AllDiff(a) => a.propagate(ctx),
        }
    }
}

/// A KenKen puzzle: a grid of candidate values together with cage and
/// all-different constraints.
///
/// ## Fixpoint invariant
///
/// A *fixpoint* is a state in which applying all constraints produces no further
/// change — every cell's candidate set is already as narrow as the constraints
/// require. Every `Puzzle` upholds this invariant: construction and mutation
/// methods propagate constraints to a fixpoint before returning. If propagation
/// would empty any cell's candidate set (a contradiction), the method returns
/// `None` instead of a `Puzzle`, so a `Puzzle` value always represents a
/// consistent, fully propagated state. Regions contribute no propagation, so
/// region-only mutations skip that step.
#[derive(Debug, Clone)]
pub struct Puzzle {
    store: Store,
    all_different: Vec<AllDifferent>,
    slots: BTreeSet<CageSlot>,
}

/// The row and column all-different constraints of an `n`×`n` grid.
fn all_different_constraints(n: usize) -> Result<Vec<AllDifferent>, Error> {
    (0..n)
        .map(|i| AllDifferent::row(n, i))
        .chain((0..n).map(|i| AllDifferent::column(n, i)))
        .collect()
}

impl Puzzle {
    /// Creates an `n`×`n` puzzle with no cages and every cell holding `1..=n`.
    ///
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`.
    pub fn new(n: usize) -> Result<Self, Error> {
        if !(1..=9).contains(&n) {
            return Err(InvalidGridSize(n));
        }
        Ok(Self {
            store: Store::full(n),
            all_different: all_different_constraints(n)?,
            slots: BTreeSet::new(),
        })
    }

    /// Creates an `n`×`n` puzzle from a set of cages, then propagates all
    /// constraints. Returns `None` if propagation finds a contradiction.
    ///
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`,
    /// [`Error::SlotNotInPuzzle`] if any cage contains a cell outside the grid,
    /// or [`Error::DuplicateSlotPolyomino`] if two cages share a polyomino.
    pub fn with_cages(n: usize, cages: &[Cage]) -> Result<Option<Self>, Error> {
        let slots: Vec<CageSlot> = cages.iter().cloned().map(CageSlot::Cage).collect();
        Self::with_slots(n, &slots)
    }

    /// Creates an `n`×`n` puzzle from a set of [`CageSlot`]s (mixed regions and
    /// cages), then propagates all constraints. Returns `None` on contradiction.
    ///
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`,
    /// [`Error::SlotNotInPuzzle`] if any slot covers a cell outside the grid, or
    /// [`Error::DuplicateSlotPolyomino`] if two slots share the same polyomino
    /// ([`CageSlot::cmp`] keys on the polyomino alone, so distinct slots over the
    /// same polyomino would silently collide in the slot set).
    pub fn with_slots(n: usize, slots: &[CageSlot]) -> Result<Option<Self>, Error> {
        if !(1..=9).contains(&n) {
            return Err(InvalidGridSize(n));
        }
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
        Ok(Self {
            store: Store::full(n),
            all_different: all_different_constraints(n)?,
            slots: slots.iter().cloned().collect(),
        }
        .propagate())
    }

    /// The number of rows or columns in the grid.
    pub const fn n(&self) -> usize {
        self.store.n()
    }

    /// Returns a new [`Puzzle`] with `cage` added, re-propagated. Returns `None`
    /// on contradiction.
    ///
    /// Idempotent: re-adding an identical cage returns the puzzle unchanged.
    ///
    /// # Errors
    /// Returns [`Error::CageConflict`] if `cage` overlaps a different cage or
    /// region already in the puzzle.
    pub fn insert(&self, cage: Cage) -> Result<Option<Self>, Error> {
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
        Ok(Self {
            store: self.store.clone(),
            all_different: self.all_different.clone(),
            slots,
        }
        .propagate())
    }

    /// Returns a new puzzle with `cage` removed and constraints re-propagated
    /// from a fully widened grid.
    ///
    /// Idempotent: removing a cage that is not present returns the puzzle
    /// unchanged.
    ///
    /// # Errors
    /// Never returns an error today; the signature mirrors the other mutators.
    pub fn remove(&self, cage: &Cage) -> Result<Self, Error> {
        let key = CageSlot::Cage(cage.clone());
        if self.slots.get(&key) != Some(&key) {
            return Ok(self.clone());
        }
        let mut slots = self.slots.clone();
        let _ = slots.remove(&key);
        Ok(self.rebuilt(slots))
    }

    /// Returns a new [`Puzzle`] with `polyomino` added as a region — a claimed
    /// shape with no operation yet. Regions contribute no propagation.
    ///
    /// Idempotent on a value-identical region already present.
    ///
    /// # Errors
    /// Returns [`Error::RegionConflict`] if `polyomino` overlaps an existing slot
    /// and is not value-identical to an existing region.
    pub fn insert_region(&self, polyomino: Polyomino) -> Result<Self, Error> {
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
        Ok(Self {
            store: self.store.clone(),
            all_different: self.all_different.clone(),
            slots,
        })
    }

    /// Returns a new [`Puzzle`] with the region at `polyomino` removed. Regions
    /// contribute no propagation, so the grid is unchanged.
    ///
    /// Idempotent. If a *cage* lives at `polyomino`, this is a no-op — callers
    /// must [`demote`](Self::demote) first.
    ///
    /// # Errors
    /// Never returns an error today; the signature mirrors [`remove`](Self::remove).
    pub fn remove_region(&self, polyomino: &Polyomino) -> Result<Self, Error> {
        let key = CageSlot::Region(polyomino.clone());
        if self.slots.get(&key) != Some(&key) {
            return Ok(self.clone());
        }
        let mut slots = self.slots.clone();
        let _ = slots.remove(&key);
        Ok(Self {
            store: self.store.clone(),
            all_different: self.all_different.clone(),
            slots,
        })
    }

    /// Returns a new [`Puzzle`] with the region at `polyomino` replaced by a cage
    /// carrying `op`, then re-propagated.
    ///
    /// Idempotent on a same-op cage already present, and a no-op on a missing
    /// polyomino.
    ///
    /// # Errors
    /// - [`Error::CageConflict`] if a cage at `polyomino` uses a different operation.
    /// - [`Error::InfeasibleOperation`] if `op` admits no valid tuples for `polyomino`.
    pub fn promote(&self, polyomino: &Polyomino, op: Operation) -> Result<Option<Self>, Error> {
        let n = u8::try_from(self.n())
            .unwrap_or_else(|_| unreachable!("Puzzle invariant: n is in 1..=9"));
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
        let _ = slots.replace(CageSlot::Cage(cage));
        Ok(Self {
            store: self.store.clone(),
            all_different: self.all_different.clone(),
            slots,
        }
        .propagate())
    }

    /// Returns a new [`Puzzle`] with the cage at `polyomino` replaced by a
    /// region, widened and re-propagated.
    ///
    /// Idempotent: a no-op when no cage lives at `polyomino`.
    ///
    /// # Errors
    /// Never returns an error today; the signature mirrors [`remove`](Self::remove).
    pub fn demote(&self, polyomino: &Polyomino) -> Result<Self, Error> {
        match self.slots.get(&CageSlot::Region(polyomino.clone())) {
            None | Some(CageSlot::Region(_)) => return Ok(self.clone()),
            Some(CageSlot::Cage(_)) => {}
        }
        let mut slots = self.slots.clone();
        let _ = slots.replace(CageSlot::Region(polyomino.clone()));
        Ok(self.rebuilt(slots))
    }

    /// Rebuilds the puzzle from a fully widened grid with `slots` and
    /// re-propagates. Used by the widening mutators ([`remove`](Self::remove),
    /// [`demote`](Self::demote)); widening can only enlarge domains, so
    /// propagation cannot contradict.
    fn rebuilt(&self, slots: BTreeSet<CageSlot>) -> Self {
        Self {
            store: Store::full(self.n()),
            all_different: self.all_different.clone(),
            slots,
        }
        .propagate()
        .unwrap_or_else(|| unreachable!("widening fills cannot produce a contradiction"))
    }

    /// The puzzle's cages in ascending polyomino order.
    pub fn cages(&self) -> impl Iterator<Item = &Cage> {
        self.slots.iter().filter_map(CageSlot::as_cage)
    }

    /// The puzzle's regions (claimed shapes with no operation yet) in ascending
    /// polyomino order.
    pub fn regions(&self) -> impl Iterator<Item = &Polyomino> {
        self.slots.iter().filter_map(CageSlot::as_region)
    }

    /// All slots (cages and regions) in ascending polyomino order.
    pub fn slots(&self) -> impl Iterator<Item = &CageSlot> {
        self.slots.iter()
    }

    /// Each cell paired with its current candidate [`Domain`], in row-major order.
    pub fn candidates(&self) -> impl Iterator<Item = (Cell, Domain)> + '_ {
        self.store.candidates()
    }

    /// Enumerates the puzzle's solutions via depth-first backtracking search.
    ///
    /// Each item is a fully solved [`Puzzle`]. The iterator is lazy: stop after
    /// the first item for a uniqueness check, or drain it to count all solutions.
    pub fn solve(&self) -> impl Iterator<Item = Self> {
        let all_different = self.all_different.clone();
        let slots = self.slots.clone();
        Search::new(self.store.clone(), self.constraints()).map(move |store| Self {
            store,
            all_different: all_different.clone(),
            slots: slots.clone(),
        })
    }

    /// Generates a random `n`×`n` puzzle using [`Puzzle::default_op_policy`] and
    /// the default cage-size distribution.
    ///
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`.
    pub fn generate<R: Rng>(n: usize, rng: &mut R) -> Result<Self, Error> {
        generate(n, rng)
    }

    /// Generates a random `n`×`n` puzzle with a caller-supplied operation policy
    /// and cage-size distribution.
    ///
    /// # Errors
    /// Returns [`Error::InvalidGridSize`] if `n` is not in `1..=9`, or any error
    /// returned by `op`.
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
    /// # Errors
    /// Returns [`Error::EmptyOpPolicyValues`] if `values` is empty.
    pub fn default_op_policy(values: &[u8], n: usize) -> Result<Operation, Error> {
        default_op_policy(values, n)
    }

    /// The constraint set: a row and a column all-different per index, plus one
    /// constraint per cage.
    fn constraints(&self) -> Vec<KenKenConstraint> {
        self.all_different
            .iter()
            .cloned()
            .map(KenKenConstraint::AllDiff)
            .chain(self.cages().cloned().map(KenKenConstraint::Cage))
            .collect()
    }

    /// Propagates all constraints to a fixed point. Returns `None` if a
    /// contradiction is found, otherwise the propagated puzzle.
    fn propagate(self) -> Option<Self> {
        let constraints = self.constraints();
        let mut store = self.store;
        let mut cache = Cache::default();
        let outcome = {
            let mut ctx = PropagationCtx::new(&mut store, &mut cache);
            propagate_to_fixpoint(&mut ctx, &constraints)
        };
        if outcome == Outcome::Contradiction || store.is_invalid() {
            return None;
        }
        Some(Self {
            store,
            all_different: self.all_different,
            slots: self.slots,
        })
    }

    #[cfg(test)]
    fn domain(&self, cell: Cell) -> Domain {
        self.store.get(cell.id())
    }

    #[cfg(test)]
    const fn store(&self) -> &Store {
        &self.store
    }
}

impl Cover for Puzzle {
    fn cells(&self) -> impl Iterator<Item = Cell> {
        self.store.cells()
    }
}

impl serde::Serialize for Puzzle {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Puzzle", 2)?;
        st.serialize_field("n", &self.n())?;
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::test_utils::{cells, l_shape, pair, singleton};

    fn puzzle_4() -> Puzzle {
        Puzzle::new(4).unwrap()
    }

    fn singleton_cage() -> Cage {
        Cage::new(4, singleton(), Operation::Given(3))
    }

    fn poly(positions: &[(usize, usize)]) -> Polyomino {
        Polyomino::from_cells(&cells(positions)).unwrap()
    }

    fn cage(n: u8, positions: &[(usize, usize)], op: Operation) -> Cage {
        Cage::new(n, poly(positions), op)
    }

    // --- new ---

    #[test]
    fn new_invalid_size_returns_err() {
        assert!(Puzzle::new(0).is_err());
        assert!(Puzzle::new(10).is_err());
    }

    #[test]
    fn new_valid_size_succeeds_with_full_cells() {
        let puzzle = Puzzle::new(3).unwrap();
        assert_eq!(puzzle.n(), 3);
        assert!(puzzle.candidates().all(|(_, f)| f == Domain::full(3)));
        assert_eq!(puzzle.cells().count(), 9);
    }

    // --- with_cages / with_slots ---

    #[test]
    fn with_cages_no_cages_is_some() {
        assert!(Puzzle::with_cages(4, &[]).unwrap().is_some());
    }

    #[test]
    fn with_cages_valid_cage_propagates() {
        let puzzle = Puzzle::with_cages(4, &[cage(4, &[(0, 0)], Operation::Given(2))])
            .unwrap()
            .unwrap();
        assert_eq!(puzzle.domain(Cell::new(0, 0)), Domain::new([2]));
    }

    #[test]
    fn with_cages_contradiction_is_none() {
        let c1 = cage(2, &[(0, 0)], Operation::Given(1));
        let c2 = cage(2, &[(0, 1)], Operation::Given(1));
        assert!(Puzzle::with_cages(2, &[c1, c2]).unwrap().is_none());
    }

    #[test]
    fn with_cages_invalid_size_is_err() {
        assert!(Puzzle::with_cages(0, &[]).is_err());
    }

    #[test]
    fn with_cages_cage_outside_grid_is_err() {
        let out = cage(4, &[(0, 0), (0, 1)], Operation::Add(3));
        assert!(matches!(
            Puzzle::with_cages(1, &[out]),
            Err(SlotNotInPuzzle(CageSlot::Cage(_)))
        ));
    }

    #[test]
    fn with_cages_duplicate_polyomino_is_err() {
        let c1 = cage(4, &[(0, 0)], Operation::Given(3));
        let c2 = cage(4, &[(0, 0)], Operation::Given(2));
        assert!(matches!(
            Puzzle::with_cages(4, &[c1, c2]),
            Err(DuplicateSlotPolyomino(_))
        ));
    }

    #[test]
    fn with_slots_mixed_region_and_cage() {
        let region = CageSlot::Region(poly(&[(0, 0)]));
        let c = CageSlot::Cage(cage(4, &[(1, 1)], Operation::Given(2)));
        let puzzle = Puzzle::with_slots(4, &[region, c]).unwrap().unwrap();
        assert_eq!(puzzle.regions().count(), 1);
        assert_eq!(puzzle.cages().count(), 1);
        assert_eq!(puzzle.domain(Cell::new(1, 1)), Domain::new([2]));
    }

    #[test]
    fn with_slots_region_outside_grid_is_err() {
        let region = CageSlot::Region(poly(&[(5, 0)]));
        assert!(matches!(
            Puzzle::with_slots(2, &[region]),
            Err(SlotNotInPuzzle(CageSlot::Region(_)))
        ));
    }

    #[test]
    fn with_slots_duplicate_polyomino_is_err() {
        let slots = [
            CageSlot::Region(singleton()),
            CageSlot::Cage(singleton_cage()),
        ];
        assert!(matches!(
            Puzzle::with_slots(4, &slots),
            Err(DuplicateSlotPolyomino(_))
        ));
    }

    // --- insert ---

    #[test]
    fn insert_non_overlapping_succeeds() {
        assert!(puzzle_4().insert(singleton_cage()).is_ok());
    }

    #[test]
    fn insert_overlapping_is_err() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let overlap = cage(4, &[(0, 0)], Operation::Given(1));
        assert!(matches!(p.insert(overlap), Err(CageConflict(_))));
    }

    #[test]
    fn insert_duplicate_is_idempotent() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let p2 = p.insert(singleton_cage()).unwrap().unwrap();
        assert_eq!(p.store(), p2.store());
    }

    #[test]
    fn insert_is_non_destructive() {
        let base = puzzle_4();
        let _ = base.insert(singleton_cage()).unwrap();
        assert!(base.insert(singleton_cage()).is_ok());
    }

    #[test]
    fn insert_contradiction_is_none() {
        let p = Puzzle::new(2)
            .unwrap()
            .insert(cage(2, &[(0, 0)], Operation::Given(1)))
            .unwrap()
            .unwrap();
        assert!(
            p.insert(cage(2, &[(0, 1)], Operation::Given(1)))
                .unwrap()
                .is_none()
        );
    }

    // --- remove ---

    #[test]
    fn remove_present_widens() {
        let c = singleton_cage();
        let p = puzzle_4().insert(c.clone()).unwrap().unwrap();
        assert_eq!(p.domain(Cell::new(0, 0)), Domain::new([3]));
        let p2 = p.remove(&c).unwrap();
        assert_eq!(p2.domain(Cell::new(0, 0)), Domain::full(4));
    }

    #[test]
    fn remove_absent_is_noop() {
        let p2 = puzzle_4().remove(&singleton_cage()).unwrap();
        assert!(p2.insert(singleton_cage()).is_ok());
    }

    #[test]
    fn remove_same_polyomino_different_operation_is_noop() {
        let original = singleton_cage();
        let p = puzzle_4().insert(original).unwrap().unwrap();
        let other = cage(4, &[(0, 0)], Operation::Given(2));
        let p2 = p.remove(&other).unwrap();
        assert_eq!(p2.domain(Cell::new(0, 0)), Domain::new([3]));
    }

    // --- insert_region / remove_region ---

    #[test]
    fn insert_region_does_not_narrow() {
        let p = puzzle_4().insert_region(singleton()).unwrap();
        assert_eq!(p.regions().count(), 1);
        assert_eq!(p.domain(Cell::new(0, 0)), Domain::full(4));
    }

    #[test]
    fn insert_region_overlap_with_cage_is_err() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        assert!(matches!(
            p.insert_region(singleton()),
            Err(RegionConflict(_))
        ));
    }

    #[test]
    fn insert_region_overlap_with_region_is_err() {
        let p = puzzle_4().insert_region(singleton()).unwrap();
        assert!(matches!(p.insert_region(pair()), Err(RegionConflict(_))));
    }

    #[test]
    fn insert_region_duplicate_is_idempotent() {
        let p = puzzle_4().insert_region(singleton()).unwrap();
        let p2 = p.insert_region(singleton()).unwrap();
        assert_eq!(p.regions().count(), p2.regions().count());
    }

    #[test]
    fn remove_region_present_and_absent() {
        let p = puzzle_4().insert_region(singleton()).unwrap();
        assert_eq!(p.remove_region(&singleton()).unwrap().regions().count(), 0);
        assert_eq!(
            puzzle_4()
                .remove_region(&singleton())
                .unwrap()
                .slots()
                .count(),
            0
        );
    }

    #[test]
    fn remove_region_on_cage_polyomino_is_noop() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let after = p.remove_region(&singleton()).unwrap();
        assert_eq!(after.cages().count(), 1);
    }

    // --- promote / demote ---

    #[test]
    fn promote_region_propagates() {
        let p = puzzle_4().insert_region(singleton()).unwrap();
        let promoted = p
            .promote(&singleton(), Operation::Given(3))
            .unwrap()
            .unwrap();
        assert_eq!(promoted.cages().count(), 1);
        assert_eq!(promoted.domain(Cell::new(0, 0)), Domain::new([3]));
    }

    #[test]
    fn promote_infeasible_operator_is_err() {
        let p = puzzle_4().insert_region(l_shape()).unwrap();
        assert!(matches!(
            p.promote(&l_shape(), Operation::Subtract(1)),
            Err(InfeasibleOperation(_, Operation::Subtract(1)))
        ));
    }

    #[test]
    fn promote_infeasible_target_is_err() {
        let p = puzzle_4().insert_region(pair()).unwrap();
        assert!(matches!(
            p.promote(&pair(), Operation::Add(1)),
            Err(InfeasibleOperation(_, Operation::Add(1)))
        ));
    }

    #[test]
    fn promote_missing_polyomino_is_noop() {
        let p2 = puzzle_4()
            .promote(&singleton(), Operation::Given(3))
            .unwrap()
            .unwrap();
        assert_eq!(p2.cages().count(), 0);
    }

    #[test]
    fn promote_existing_same_op_is_idempotent() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let p2 = p
            .promote(&singleton(), Operation::Given(3))
            .unwrap()
            .unwrap();
        assert_eq!(p.store(), p2.store());
    }

    #[test]
    fn promote_existing_different_op_is_conflict() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        assert!(matches!(
            p.promote(&singleton(), Operation::Given(2)),
            Err(CageConflict(_))
        ));
    }

    #[test]
    fn demote_widens_and_replaces_with_region() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let p2 = p.demote(&singleton()).unwrap();
        assert_eq!(p2.cages().count(), 0);
        assert_eq!(p2.regions().count(), 1);
        assert_eq!(p2.domain(Cell::new(0, 0)), Domain::full(4));
    }

    #[test]
    fn demote_absent_and_already_region_are_noops() {
        assert_eq!(puzzle_4().demote(&singleton()).unwrap().slots().count(), 0);
        let p = puzzle_4().insert_region(singleton()).unwrap();
        assert_eq!(p.demote(&singleton()).unwrap().regions().count(), 1);
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
        assert_eq!(p.store(), p2.store());
    }

    // --- accessors / ordering ---

    #[test]
    fn cages_and_slots_are_in_polyomino_order() {
        let a = cage(4, &[(0, 0)], Operation::Given(3));
        let b = cage(4, &[(1, 1)], Operation::Given(2));
        let puzzle = puzzle_4()
            .insert(b)
            .unwrap()
            .unwrap()
            .insert(a.clone())
            .unwrap()
            .unwrap();
        assert_eq!(puzzle.cages().next(), Some(&a));
        assert_eq!(puzzle.slots().count(), 2);
    }

    #[test]
    fn regions_yields_only_regions() {
        let p = puzzle_4()
            .insert(singleton_cage())
            .unwrap()
            .unwrap()
            .insert_region(poly(&[(1, 1)]))
            .unwrap();
        assert_eq!(p.regions().count(), 1);
        assert_eq!(p.cages().count(), 1);
    }

    #[test]
    fn candidates_are_row_major_and_reflect_pins() {
        let puzzle = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let cells: Vec<Cell> = puzzle.candidates().map(|(c, _)| c).collect();
        assert_eq!(cells.first(), Some(&Cell::new(0, 0)));
        assert_eq!(cells.len(), 16);
        let pinned = puzzle
            .candidates()
            .find(|(c, _)| *c == Cell::new(0, 0))
            .unwrap()
            .1;
        assert_eq!(pinned, Domain::new([3]));
    }

    // --- internal engine ---

    #[test]
    fn propagate_is_idempotent() {
        let p = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let again = p.clone().propagate().unwrap();
        assert_eq!(p.store(), again.store());
    }

    // --- solve ---

    #[test]
    fn solve_unique_puzzle_yields_one_solution() {
        let cages = [
            cage(2, &[(0, 0)], Operation::Given(1)),
            cage(2, &[(0, 1)], Operation::Given(2)),
        ];
        let puzzle = Puzzle::with_cages(2, &cages).unwrap().unwrap();
        let solutions: Vec<Puzzle> = puzzle.solve().collect();
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].domain(Cell::new(1, 1)), Domain::new([1]));
    }

    #[test]
    fn solve_empty_3x3_has_twelve_latin_squares() {
        assert_eq!(puzzle_4().solve().count(), 576);
        assert_eq!(Puzzle::new(3).unwrap().solve().count(), 12);
    }

    // --- serde ---

    #[test]
    fn serializes_to_n_and_slots() {
        let puzzle = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let json = serde_json::to_string(&puzzle).unwrap();
        assert!(json.contains("\"n\":4"));
        assert!(json.contains("\"slots\""));
    }

    #[test]
    fn round_trips_through_json() {
        let original = puzzle_4().insert(singleton_cage()).unwrap().unwrap();
        let restored: Puzzle =
            serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
        assert_eq!(original.store(), restored.store());
        itertools::assert_equal(original.cages(), restored.cages());
    }

    #[test]
    fn round_trips_with_regions_preserved() {
        let original = Puzzle::with_slots(
            4,
            &[
                CageSlot::Region(poly(&[(0, 0)])),
                CageSlot::Cage(cage(4, &[(1, 1)], Operation::Given(2))),
            ],
        )
        .unwrap()
        .unwrap();
        let restored: Puzzle =
            serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
        assert_eq!(
            original.slots().collect::<Vec<_>>(),
            restored.slots().collect::<Vec<_>>()
        );
        assert_eq!(original.store(), restored.store());
    }

    #[test]
    fn serialize_format_locks_shape() {
        let puzzle = Puzzle::with_slots(
            2,
            &[CageSlot::Cage(cage(2, &[(1, 1)], Operation::Given(2)))],
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            serde_json::to_value(&puzzle).unwrap(),
            serde_json::json!({
                "n": 2,
                "slots": [
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
    fn deserialize_rejects_contradiction_and_bad_size() {
        let contradiction = r#"{"n":2,"slots":[
            {"Cage":{"polyomino":[{"row":0,"column":0}],"operation":{"Given":1},"n":2}},
            {"Cage":{"polyomino":[{"row":0,"column":1}],"operation":{"Given":1},"n":2}}
        ]}"#;
        assert!(serde_json::from_str::<Puzzle>(contradiction).is_err());
        assert!(serde_json::from_str::<Puzzle>(r#"{"n":99,"slots":[]}"#).is_err());
        let out_of_grid = r#"{"n":2,"slots":[{"Region":[{"row":5,"column":0}]}]}"#;
        assert!(serde_json::from_str::<Puzzle>(out_of_grid).is_err());
    }
}
