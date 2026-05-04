# KenKen

[![CI](https://github.com/wpm/KenKen/actions/workflows/ci.yml/badge.svg)](https://github.com/wpm/KenKen/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/wpm/KenKen/branch/main/graph/badge.svg)](https://codecov.io/gh/wpm/KenKen)

A Rust library for generating and solving KenKen puzzles.

[KenKen](https://www.kenken.com) is a math-based logic puzzle played on an n×n grid. The grid is divided
into cages, each labeled with a target number and an arithmetic operation. The goal is to fill the grid
with digits 1 through n such that no digit repeats in any row or column, and the numbers in each cage
produce the target value using the given operation.

## What the library provides

**Generate puzzles** with `generate(n, rng)` for a random n×n puzzle, or
`generate_with(n, rng, op_policy, size_distribution)` to control how cage operations are assigned and
how cage sizes are distributed.

**Query solution counts** on any `Puzzle`:

- `uniqueness()` — classifies the puzzle as having no solution, exactly one, or more than one; stops
  the solver early once a second solution is found.
- `solutions()` — exhaustive count of all solutions.
- `solutions_at_most(k)` — count up to `k` solutions; useful for capping runtime when only a threshold
  matters.

**Customize generation** via two named knobs:

- `default_op_policy` / custom closure — maps a cage's cell values to an `Operation` (`Add`,
  `Subtract`, `Multiply`, `Divide`, or `Given`).
- `DEFAULT_SIZE_DISTRIBUTION` / `SizeDistribution` — controls cage-size sampling (`Fixed(n)` or
  `Uniform { min, max }`).
