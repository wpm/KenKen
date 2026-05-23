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

Everything is accessed through the [`Puzzle`] type.

**Build puzzles** with `Puzzle::new(n)`, then add cages via `insert(cage)` and remove them
via `remove(&cage)`, or bulk-construct with `Puzzle::with_cages(n, &cages)`. Each `Cage` is a
`Polyomino` paired with an `Operation` (`Add`, `Subtract`, `Multiply`, `Divide`, or `Given`).

**Enumerate solutions** with `puzzle.solutions()`, which returns `None` if the puzzle is
incomplete, `Some([])` if it is complete but unsatisfiable, or `Some([…])` with all solutions.
The result is lazily computed on the first call and cached for subsequent calls. Use
`puzzle.solution_count()` to get only the count.

**Generate puzzles** randomly with `Puzzle::generate(n, rng)`, or use
`Puzzle::generate_with(n, rng, op_policy, sizes)` for custom operation assignment and cage-size
distribution. `Puzzle::default_op_policy` is exposed so callers can compose it with their own
overrides.
