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

## Generation

The library can also generate random KenKen puzzles with a **guaranteed unique solution**, via `generate(size, seed) -> (Puzzle, solution)`. The algorithm uses **coarsening**: it first builds a uniformly random Latin square as the target solution (see below), then starts from a maximally-constrained puzzle (one `Given` cage per cell, trivially unique) and repeatedly merges pairs of adjacent cages, assigning each merged cage an operation consistent with the solution. A candidate merge is kept only if the resulting puzzle still has exactly one solution; otherwise the pair is blacklisted and another is tried. This is strictly more efficient than generate-and-test: every intermediate state is a valid uniquely-solvable puzzle, each step is a local perturbation, and rejection of a merge carries information (those two cages together would lose uniqueness).

Uniqueness is checked via `has_unique_solution`, which runs the backtracking search but stops as soon as a second solution is found.

### Random Latin square

The target solution is sampled (approximately) uniformly at random from the set of *n*×*n* Latin squares using the **Jacobson–Matthews** Markov chain[^jm]. The chain's state is the *n*×*n*×*n* incidence cube of the square; each step perturbs eight cells of a 2×2×2 sub-cube by ±1, alternating between *proper* (all-{0,1}) Latin-square states and *improper* states with a single −1 entry. Restricted to proper states the stationary distribution is uniform, so mixing for a few *n*³ steps yields an essentially uniform sample. This is a substantial improvement over uniform-random row-by-row backtracking, which biases heavily toward squares that happen to extend easily.

[^jm]: Mark T. Jacobson and Peter Matthews, "Generating uniformly distributed random Latin squares", *Journal of Combinatorial Designs* 4(6), 1996, pp. 405–437. <https://doi.org/10.1002/(SICI)1520-6610(1996)4:6%3C405::AID-JCD3%3E3.0.CO;2-J>

## Validation

Separate from solving, the library can validate:

- **Solutions**: a completed grid satisfies all Latin square and cage constraints.
- **Cage contiguity**: the cells of a cage form a single orthogonally connected region.
- **Puzzle coverage**: the cages exactly tile the grid — every cell belongs to exactly one cage, with no overlaps or out-of-bounds cells.
