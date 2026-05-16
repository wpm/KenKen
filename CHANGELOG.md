# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-05-16

### Added
- `Solver<S>` and `State` trait are now public, allowing direct solution enumeration
  via `Solver::new(puzzle)`.
- `Puzzle::insert` and `Puzzle::remove` replace the old `insert_cage`/`remove_cage`
  methods; `insert` now returns `Result<Option<Puzzle>>` (propagation may detect a
  contradiction).
- `Puzzle::propagate` (fixed-point constraint propagation) and `Puzzle::branch` (MRV
  branching heuristic) are now the primary solver primitives.
- `Polyomino::intersects` — tests whether two polyominoes share any cell.
- `generate` / `generate_with` — random puzzle generation with pluggable operation
  policy and `SizeDistribution`.
- `Cover` trait — implemented by `AllDifferent`, `Cage`, `Polyomino`, and `Puzzle`.

### Changed
- `Constraint` trait redesigned: `apply_to(&Grid) -> Result<Grid, Error>` replaces the
  old `AllowedValues`-based callback model. Constraints are now composable via
  `try_fold` and all constraints are monotone filters (candidates are only removed,
  never added).
- `Cover::cells` now returns `impl Iterator<Item = Cell>` instead of `Vec<Cell>`.
- `Polyomino::new` renamed to `Polyomino::from_cells`.
- `State::propagate` now returns `Result<Option<Self>, Error>` (was `Option<Self>`);
  the `Err` path surfaces grid-access errors from out-of-bounds constraints.
- CI coverage gate lowered to 99% — the `continue` branch in `greedy`'s frontier
  dedup and the `unreachable!()` path in `generate_with` are structurally unreachable
  in practice.

### Removed
- `Puzzle::uniqueness()`, `Puzzle::solutions()`, `Puzzle::solutions_at_most(k)` —
  equivalent functionality is available by driving `Solver::new(puzzle)` directly.
- `Puzzle::rank_tuples_for_cage()` and `NarrowingScore` — internal tuning API with no
  replacement; cage constraint strength is now implicit in propagation.
- `Puzzle::narrow()`, `Puzzle::widen()`, `Puzzle::propagate_fully()` — superseded by
  `Puzzle::propagate`.
- `Puzzle::singleton_cells()`, `Puzzle::empty_cells()`, `Puzzle::candidates()` —
  equivalent information is available via `Puzzle::cells()` and `Grid` accessors.
- `Polyomino::new_unchecked()` — removed without replacement.
- `M`, `Index`, and `N` type aliases removed from the public API; signatures now use
  `u16`, `usize`, and `u8` directly.
- `itertools` dependency removed.

### Internal
- `Error::FlipWouldDisconnect` renamed to `Error::WouldDisconnect`.
- Polyomino logic moved from `cover.rs` to a dedicated `constraints/polyomino.rs`
  module.
- Shared test fixtures (`singleton`, `pair`, `l_shape`, etc.) consolidated into
  `constraints::test_utils`.

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
