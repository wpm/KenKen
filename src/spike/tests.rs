//! End-to-end tests on the shared fixed instance, asserting correctness parity
//! with the production solver/propagator.

#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;

use crate::{
    Cell, Operation, Polyomino, Puzzle,
    spike::{
        KenKenConstraint,
        all_different::AllDiffDef,
        cage::CageDef,
        constraint::Constraint,
        fixtures,
        problem::{PartialPuzzle, fixpoint},
        solver::{BacktrackSolver, Solver},
        store::Store,
        variable::Variable,
    },
    types::N,
};

/// A solved store as a canonical cell→value assignment.
fn store_assignment(store: &Store) -> Vec<(Cell, N)> {
    store
        .cells()
        .map(|cell| (cell, store.get(cell.id()).iter().next().unwrap()))
        .collect()
}

/// A solved puzzle as a canonical cell→value assignment.
fn puzzle_assignment(puzzle: &Puzzle) -> Vec<(Cell, N)> {
    puzzle
        .candidates()
        .map(|(cell, fill)| (cell, fill.iter().next().unwrap()))
        .collect()
}

#[test]
fn kenken_constraint_dispatches_variables() {
    let poly = Polyomino::from_cells(&[Cell::new(0, 0)]).unwrap();
    let cage = KenKenConstraint::Cage(CageDef::new(2, poly, Operation::Given(1)));
    assert_eq!(cage.variables(), &[Cell::new(0, 0)]);

    let all_diff = KenKenConstraint::AllDiff(AllDiffDef::row(2, 0));
    assert_eq!(all_diff.variables(), &[Cell::new(0, 0), Cell::new(0, 1)]);
}

#[test]
fn fixpoint_matches_production_propagation() {
    let puzzle = fixtures::puzzle();
    let store = fixpoint(&fixtures::partial(&puzzle));
    assert!(!store.is_invalid());
    for (cell, fill) in puzzle.candidates() {
        assert_eq!(store.get(cell.id()), fill, "fixpoint diverges at {cell:?}");
    }
}

#[test]
fn solver_matches_production_solutions() {
    let puzzle = fixtures::puzzle();
    let result = BacktrackSolver::new().solve(&fixtures::partial(&puzzle).problem());

    let spike: BTreeSet<Vec<(Cell, N)>> = result.solutions().iter().map(store_assignment).collect();
    let production: BTreeSet<Vec<(Cell, N)>> =
        puzzle.solve().map(|p| puzzle_assignment(&p)).collect();

    assert!(!spike.is_empty(), "spike found no solutions");
    assert_eq!(spike, production);
}

#[test]
fn fixpoint_on_partial_subset_is_consistent() {
    // A genuine mid-design partial state — only some cages placed, plus a
    // committed given — is the Designer's use case for `fixpoint`.
    let puzzle = fixtures::puzzle();
    let mut partial = PartialPuzzle::new(puzzle.n());
    for cage in puzzle.cages().take(3) {
        partial = partial.with_cage(CageDef::new(
            cage.n(),
            cage.polyomino().clone(),
            cage.operation(),
        ));
    }
    let store = fixpoint(&partial);
    assert!(!store.is_invalid());
    // Propagation alone need not solve a partial instance, but it must stay
    // consistent and remain a valid superset of the production solution.
    let solution = puzzle_assignment(&puzzle.solve().next().unwrap());
    for (cell, value) in solution {
        assert!(
            !(store.get(cell.id()) & crate::Fill::new([value])).is_empty(),
            "partial fixpoint pruned a solution value at {cell:?}"
        );
    }
}
