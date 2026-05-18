# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `Polyomino::cells()` — iterates the polyomino's cells in row-major order.
- `Polyomino::len()` — number of cells (O(1)).
- `Polyomino::contains(Cell)` — membership test (O(log n)).
- `Polyomino::is_edge_connected_component(&[Cell])` — associated function
  exposing the edge-connectivity check used during polyomino construction.
- `Cage::cells()` and `Cage::len()` — accessors that delegate to the underlying
  polyomino.
- `Puzzle::with_slots(n, &[CageSlot])` — bulk constructor accepting mixed
  `Region` and `Cage` slots, the generalization of `with_cages`. Validates
  slot bounds and rejects duplicate polyominoes, then propagates to fixpoint.
- `Error::DuplicateSlotPolyomino(Polyomino)` — raised by `with_slots` (and the
  `Puzzle` deserializer) when two slots share the same polyomino.

### Changed
- `Puzzle::with_cages` is now a thin wrapper around `Puzzle::with_slots`.
- `Error::CageNotInPuzzle(Cage)` renamed to `Error::SlotNotInPuzzle(CageSlot)`
  to cover both cage and region out-of-bounds cases.

## [0.3.0] - 2026-05-17

### Added
- `Puzzle::solve` — enumerates solutions as an iterator (replaces the previously
  public `Solver` / `State` types).
- `Puzzle::generate` and `Puzzle::generate_with` — random puzzle generation
  exposed as methods on `Puzzle` (the free `generate` / `generate_with`
  functions are no longer public). Both return `Result<Puzzle, Error>`; the
  previous `Result<Option<Puzzle>, Error>` had a vestigial `None` arm.
- `Puzzle::default_op_policy` — the default cage-operation policy, exposed for
  composition with `Puzzle::generate_with`.
- `Puzzle::with_cages(n, &[Cage])` — bulk constructor that validates cage bounds,
  then propagates all constraints to fixpoint. Returns `None` if the cages
  produce a contradiction.
- `Puzzle::n()` — returns the grid size.
- `Puzzle::cages()` — iterates over the puzzle's cages in sorted order.
- `Puzzle::insert` and `Puzzle::remove` replace the old `insert_cage`/`remove_cage`
  methods; `insert` returns `Result<Option<Puzzle>>` (propagation may detect a
  contradiction); `remove` resets the full grid before re-propagating so
  `AllDifferent` cascade effects are correctly unwound.
- `Polyomino::intersects` — tests whether two polyominoes share any cell.
- `serde` support: `Puzzle`, `Cage`, `Polyomino`, `Operation`, `Cell`, and `Fill`
  all implement `Serialize` and `Deserialize`. `Puzzle` serializes as
  `{"n": …, "cages": […]}`; the propagated grid is reconstructed on
  deserialization.

### Changed
- `Puzzle::new(n)` creates an empty `n`×`n` puzzle (no cages). The previous
  `new(grid, cages)` signature is superseded by `with_cages`.
- `SizeDistribution` replaced the fixed/uniform cage-size scheme with a
  Poisson distribution (mean `n/3`, clamped to `1..=n`) that better matches
  the irregular cage sizes of published KenKen puzzles.
- `Constraint` trait redesigned: `apply_to(&Grid) -> Result<Grid, Error>` replaces
  the old `AllowedValues`-based callback model. Constraints are now composable via
  `try_fold` and are monotone filters (candidates are only removed, never added).
- `Polyomino::new` renamed to `Polyomino::from_cells`.
- CI coverage gate lowered to 99% — the `continue` branch in `greedy`'s frontier
  dedup and the `unreachable!()` path in `generate_with` are structurally
  unreachable in practice.

### Removed
- `Solver<S>` and the `State` trait — collapsed into `Puzzle::solve`.
- Free functions `generate`, `generate_with`, and `default_op_policy` — collapsed
  into methods on `Puzzle`.
- `Grid`, `Fill`, the `Cover` trait, the `Constraint` trait, and the
  `constraints` module are no longer part of the public API; they remain
  crate-internal implementation details.
- `Puzzle::uniqueness()`, `Puzzle::solutions()`, `Puzzle::solutions_at_most(k)` —
  equivalent functionality is available via `Puzzle::solve()`.
- `Puzzle::rank_tuples_for_cage()` and `NarrowingScore` — internal tuning API with no
  replacement; cage constraint strength is now implicit in propagation.
- `Puzzle::narrow()`, `Puzzle::widen()`, `Puzzle::propagate_fully()` — propagation
  now runs implicitly inside `with_cages`, `insert`, and `remove`.
- `Puzzle::singleton_cells()`, `Puzzle::empty_cells()`, `Puzzle::candidates()` —
  no replacement; solved puzzles are now returned directly by `Puzzle::solve`.
- `Polyomino::new_unchecked()` — removed without replacement.
- `M`, `Index`, and `N` type aliases removed from the public API; signatures now use
  `u16`, `usize`, and `u8` directly.
- `Error::DeltaSizeMismatch` — the `Delta` API was removed earlier in 0.3.0
  development, leaving this variant unreachable.
- `itertools` dependency removed (now a dev-dependency only).

### Internal
- `Error::FlipWouldDisconnect` renamed to `Error::WouldDisconnect`.
- Polyomino logic moved from `cover.rs` to a dedicated `constraints/polyomino.rs`
  module.
- Shared test fixtures (`singleton`, `pair`, `l_shape`, etc.) consolidated into
  `constraints::test_utils`.
- `AllDifferent` constraint construction moved into `Grid::all_different_constraints()`.

## [0.1.0] - 2026-05-12

### Added
- `Puzzle` — core type for building and querying KenKen puzzles.
  - `Puzzle::new(n)`, `insert_cage`, `remove_cage`
  - `uniqueness()`, `solutions()`, `solutions_at_most(k)`
  - `propagate_fully()`, `narrow()`, `widen()`
  - `rank_tuples_for_cage()` with `NarrowingScore`
  - `singleton_cells()`, `empty_cells()`, `candidates()`
- `Cage` — a `Polyomino` paired with an `Operation`.
  - `Cage::new(n, polyomino, operation)`
  - `valid_operators()`, `valid_targets()`, `is_valid()`
- `Polyomino` — a set of contiguous grid cells.
  - `extend()`, `without()`
- `Operation` and `Operator` enums (`Add`, `Subtract`, `Multiply`, `Divide`, `Given`).
- `operator_tuples(n, polyomino, operator)` — enumerates all valid operations and their
  tuples for a given operator on a polyomino.
- `generate(n, rng)` and `generate_with(n, rng, op_policy, size_distribution)` for
  random puzzle generation.
- `SizeDistribution` (`Fixed`, `Uniform`) for controlling cage-size sampling.
- `Delta` for applying candidate-domain updates to a `Puzzle`.
- `Grid`, `Values`, `Cell` — grid and candidate-value types.
- `Uniqueness` (`None`, `Unique`, `Multiple`) — solution-count classification.
