# Spike comparison criteria: CS-trait vs. Pumpkin (issue #106)

**Status: pre-registered.** This document is written *before* the two spikes are
evaluated against each other, so the comparison cannot be quietly reshaped to
favour whichever result we happen to get. It records what we are measuring and
how, plus what the CS-trait spike actually produced.

This is a spike artifact. Nothing here ships, and nothing in this branch
(`spike/cs-trait`) merges to `main`. When the comparison is done, the findings
are distilled into a separate design document or follow-up issue; the spike
branches stay as reference artifacts.

## What the CS-trait spike built

A self-contained module (`src/spike/`, gated to `#[cfg(test)]`) that reorganizes
one end-to-end slice of the solver around standard CS nomenclature, layered over
the existing KenKen domain types (`Cell`, `Polyomino`, `Operation`, `Fill`):

- `Variable` (implemented on `Cell`), `Constraint<V>`, `Solver<V, C>`, and the
  free function `propagate_to_fixpoint` — matching the signatures in the issue.
- **Store vs. cache** separation: `Store` holds intrinsic per-cell domains and
  is cheap to clone (one boxed slice); `Cache` memoizes derived viable-tuple
  sets, keyed on `(cage, projected domains)`.
- `viable_tuples(cage, store, cache)` — a **pure** function; the cache is a
  transparent memo, never a source of truth. Clearing it changes only
  performance (proved by `cache::tests::cache_is_pure_clearing_does_not_change_result`).
- `CageDef` carries **no** tuple state — the structural fix for the historical
  `Cage.tuples` smell. All tuple queries go through `viable_tuples`.
- Two interchangeable all-different propagators behind the
  `AllDifferentPropagator` trait: `ReginAllDifferent` (full Régin, **with** the
  free-value sink the production `regin()` omits — see issue #30) and
  `BruteForceAllDifferent` (the GAC oracle).
- `BacktrackSolver` (plain DFS) and the Designer entry point `fixpoint(partial)`.

## The fixed test instance

Both spikes must use the **same** instance: a 6×6 KenKen produced by the
production generator at seed `20_260_521` (`spike::fixtures`). Fixing it prevents
the comparison from sliding into "well, on a different example...". The Pumpkin
spike must encode and solve this exact instance.

## Pre-registered comparison criteria

1. **Correctness parity** — same solution set, same fixpoint on the partial
   state. *CS-trait result:* the spike's `fixpoint` reproduces the production
   propagator's `candidates()` cell-for-cell, and `BacktrackSolver` reproduces
   the production solver's full solution set on the fixed instance
   (`spike::tests`). If the Pumpkin fixpoint diverges, that is the
   encoding-strength question and it matters.
2. **Performance** — wall-clock on the fixed instance plus a small random
   population. *Direction, not finality.* (Not yet benchmarked; the spike is
   shaped for a fair later measurement, no premature optimization.)
3. **Surface area** — how much API the rest of the codebase must call. *CS-trait:*
   `Variable`/`Constraint`/`Solver`/`propagate_to_fixpoint`, `Store`/`Cache`,
   `viable_tuples`, `fixpoint`. One concrete impl per trait (plus the explicit
   `AllDifferent` two-impl carve-out).
4. **Designer coupling** — given the `fixpoint` output, how clean is the path to
   the tuple-highlight feature. *CS-trait:* `fixpoint(&PartialPuzzle) -> Store`
   plus `viable_tuples(cage, store, cache)` give the highlight feature exactly a
   reduced store and a pure per-cage tuple query. The Pumpkin spike needs an
   equivalent propagation-without-search operation; if it can't expose one,
   that is a real finding.
5. **Code clarity** — read the solve-a-puzzle path on both sides; which would
   you rather maintain.
6. **Dependency footprint** — build complexity, version pressure, external
   commitments. *CS-trait:* zero new dependencies (reuses `Fill`, `Polyomino`,
   `Operation`, and the existing seeded-RNG test stack).

## Side finding: is the existing `regin()` actually Régin?

The property test (`all_different::tests::regin_matches_brute_force_oracle`)
runs 5000 random instances across the full ≤8-variable / ≤8-value regime and
asserts the full-Régin spike implementation agrees with the brute-force oracle —
including the value > variable cases the production `regin()` gets wrong. The
spike's full Régin passes. This confirms the production `regin()` is **Régin
restricted to its stated precondition** (values ≤ variables, always true for
n×n row/column constraints) but not full GAC Régin in general; the missing piece
is exactly the free-value sink / reachability step, as issue #30 records.
