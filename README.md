# KenKen

[![CI](https://github.com/wpm/KenKen/actions/workflows/ci.yml/badge.svg)](https://github.com/wpm/KenKen/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/wpm/KenKen/branch/main/graph/badge.svg)](https://codecov.io/gh/wpm/KenKen)

A Rust library for generating and solving KenKen puzzles.

[KenKen](https://www.kenken.com) is a math-based logic puzzle played on an n×n grid. The grid is divided
into cages, each labeled with a target number and an arithmetic operation. The goal is to fill the grid
with digits 1 through n such that no digit repeats in any row or column, and the numbers in each cage
produce the target value using the given operation.

## Development setup

After cloning, configure git to use the checked-in hooks:

```sh
git config core.hooksPath .githooks
```

## What the library provides

**Build puzzles** with `Puzzle::new(n)`, then add cages via `insert_cage` and remove them via
`remove_cage`. Each `Cage` is a `Polyomino` paired with an `Operation` (`Add`, `Subtract`,
`Multiply`, `Divide`, or `Given`).

**Query solution counts** on any `Puzzle`:

- `uniqueness()` — classifies the puzzle as having no solution, exactly one, or more than one; stops
  the solver early once a second solution is found.
- `solutions()` — exhaustive count of all solutions.
- `solutions_at_most(k)` — count up to `k` solutions; useful for capping runtime when only a threshold
  matters.

**Enumerate solutions** directly with `Solver::new(puzzle)`, a depth-first backtracking iterator
that yields one solved `Puzzle` per solution.

**Generate puzzles** randomly with `generate(n, rng)`, or use `generate_with(n, rng, op_policy,
size_distribution)` to control how cage operations are assigned and how cage sizes are distributed:

- `default_op_policy` / custom closure — maps a cage's cell values to an `Operation`.
- `DEFAULT_SIZE_DISTRIBUTION` / `SizeDistribution` — controls cage-size sampling (`Fixed(n)` or
  `Uniform { min, max }`).
