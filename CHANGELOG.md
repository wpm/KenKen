# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `Solver<S>` and `State` trait are now public, allowing direct solution enumeration
  via `Solver::new(puzzle)`.

### Changed
- `M`, `Index`, and `N` type aliases removed from the public API; signatures now use
  `u16`, `usize`, and `u8` directly.
- `Error::FlipWouldDisconnect` renamed to `Error::WouldDisconnect`.
- `Constraint::value_filter` and `ValueFilter::apply` are now infallible; the previous
  `Result` returns were never observed in practice and represented unreachable paths
  for any puzzle constructed via `Puzzle::new`.

### Internal
- Test coverage raised to 100% (LCOV line coverage). CI now gates new code on
  `cargo llvm-cov --fail-under-lines 100`.

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
