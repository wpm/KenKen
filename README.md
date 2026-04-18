# KenKen

A library for solving [KenKen](https://en.wikipedia.org/wiki/KenKen) puzzles.

## What is KenKen?

KenKen is a numeric logic puzzle played on an *n*×*n* grid. Each row and column must contain each integer from 1 to *n* exactly once (a Latin square). The grid is partitioned into **cages** — contiguous groups of cells — each labeled with a target number and an arithmetic operation. The values in each cage must combine under that operation to reach its target.

## Solving

The solver uses **backtracking with constraint propagation**. It fills cells one at a time and checks two classes of constraints after each placement:

- **Latin square**: no value may appear twice in the same row or column.
- **Cage feasibility**: for each cage that contains the just-placed cell, the current partial values must still be consistent with the cage's operation and target. Fully filled cages are checked for exact satisfaction; partially filled cages are pruned early when the partial result already makes the target unreachable.

When a placement violates either constraint the solver backtracks immediately, avoiding large branches of the search tree.

Two entry points are provided: one that returns the first solution found (short-circuiting as soon as one is complete), and one that exhausts the search tree to collect every solution.

## Validation

Separate from solving, the library can validate:

- **Solutions**: a completed grid satisfies all Latin square and cage constraints.
- **Cage contiguity**: the cells of a cage form a single orthogonally connected region.
- **Puzzle coverage**: the cages exactly tile the grid — every cell belongs to exactly one cage, with no overlaps or out-of-bounds cells.
