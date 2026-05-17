# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-05-16

### Added
- `Puzzle::solve` — enumerates solutions as an iterator (replaces the previously
  public `Solver` / `State` types).
- `Puzzle::generate` and `Puzzle::generate_with` — random puzzle generation
  exposed as methods on `Puzzle` (the free `generate` / `generate_with`
  functions are no longer public). Both return `Result<Puzzle, Error>`; the
  previous `Result<Option<Puzzle>, Error>` had a vestigial `None` arm.
- `Puzzle::default_op_policy` — the default cage-operation policy, exposed for
  composition with `Puzzle::generate_with`.
- `Puzzle::with_cages(n, &[Cage])` — bulk constructor that no longer requires
  the caller to build a `Grid` first.
- `Puzzle::insert` and `Puzzle::remove` replace the old `insert_cage`/`remove_cage`
  methods; `insert` now returns `Result<Option<Puzzle>>` (propagation may detect a
  contradiction).
- `Polyomino::intersects` — tests whether two polyominoes share any cell.

### Changed
- `Constraint` trait redesigned: `apply_to(&Grid) -> Result<Grid, Error>` replaces the
  old `AllowedValues`-based callback model. Constraints are now composable via
  `try_fold` and all constraints are monotone filters (candidates are only removed,
  never added).
- `Polyomino::new` renamed to `Polyomino::from_cells`.
- CI coverage gate lowered to 99% — the `continue` branch in `greedy`'s frontier
  dedup and the `unreachable!()` path in `generate_with` are structurally unreachable
  in practice.

### Removed
- `Solver<S>` and the `State` trait — collapsed into `Puzzle::solve`.
- Free functions `generate`, `generate_with`, and `default_op_policy` — collapsed
  into methods on `Puzzle`.
- `Puzzle::new(grid, cages)` — superseded by `Puzzle::with_cages(n, cages)`,
  which no longer requires the caller to construct a `Grid`.
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
