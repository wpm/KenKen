//! Integration tests for the crate's public API. These run as an external user
//! would, guarding against accidental visibility regressions.

#![allow(clippy::unwrap_used)]
mod test {

    use kenken::{Cage, Cell, Fill, Operation, Polyomino, Puzzle, SizeDistribution};
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
    }

    const fn cell(row: usize, column: usize) -> Cell {
        Cell::new(row, column)
    }

    /// Builds a [`Cage`] over `cells` (given as `(row, column)` pairs) for an
    /// `n`-sized grid.
    fn cage(n: u8, cells: &[(usize, usize)], op: Operation) -> Cage {
        let cells: Vec<Cell> = cells.iter().map(|&(r, c)| cell(r, c)).collect();
        Cage::new(n, Polyomino::from_cells(&cells).unwrap(), op)
    }

    /// A 6×6 puzzle fully covered by 18 vertical `Subtract(1)` pair cages,
    /// one per column pair `((0,c)-(1,c))`, `((2,c)-(3,c))`, `((4,c)-(5,c))`.
    ///
    /// Each cage admits both orderings of every consecutive pair, so many
    /// Latin-square completions satisfy the constraints.
    fn multi_solution_puzzle() -> Puzzle {
        let mut cages = Vec::with_capacity(18);
        for col in 0..6 {
            for (r0, r1) in [(0, 1), (2, 3), (4, 5)] {
                cages.push(cage(6, &[(r0, col), (r1, col)], Operation::Subtract(1)));
            }
        }
        Puzzle::with_cages(6, &cages).unwrap().unwrap()
    }

    /// The [`multi_solution_puzzle`] layout with one cage swapped from
    /// `Subtract(1)` to `Subtract(4)`. Initial propagation narrows
    /// `(0,0)` and `(1,0)` to `{1, 2, 5, 6}` but cannot see the global
    /// infeasibility: every assignment of column 0 leaves the remaining
    /// four cells as `{2,3,4,6}` or `{1,3,4,5}`, neither of which can be
    /// partitioned into two consecutive-integer pairs for the
    /// `(2,0)-(3,0)` and `(4,0)-(5,0)` `Subtract(1)` cages.
    fn no_solution_puzzle() -> Puzzle {
        let mut cages = Vec::with_capacity(18);
        cages.push(cage(6, &[(0, 0), (1, 0)], Operation::Subtract(4)));
        for col in 0..6 {
            let row_pairs: &[(usize, usize)] = if col == 0 {
                &[(2, 3), (4, 5)]
            } else {
                &[(0, 1), (2, 3), (4, 5)]
            };
            for &(r0, r1) in row_pairs {
                cages.push(cage(6, &[(r0, col), (r1, col)], Operation::Subtract(1)));
            }
        }
        Puzzle::with_cages(6, &cages).unwrap().unwrap()
    }

    #[test]
    fn solve_finds_multiple_solutions_for_underconstrained_puzzle() {
        let puzzle = multi_solution_puzzle();
        let start = std::time::Instant::now();
        let mut solutions = puzzle.solve();
        assert!(solutions.next().is_some());
        assert!(solutions.next().is_some());
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn solve_finds_no_solutions_for_infeasible_puzzle() {
        let puzzle = no_solution_puzzle();
        let start = std::time::Instant::now();
        let mut solutions = puzzle.solve();
        assert!(solutions.next().is_none());
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn solve_finds_exactly_one_solution_for_uniquely_determined_puzzle() {
        // 2×2 puzzle with Given(1) at (0,0) and Given(2) at (0,1) admits only
        // the Latin completion (1 2 / 2 1).
        let cages = [
            cage(2, &[(0, 0)], Operation::Given(1)),
            cage(2, &[(0, 1)], Operation::Given(2)),
        ];
        let puzzle = Puzzle::with_cages(2, &cages).unwrap().unwrap();
        assert_eq!(puzzle.solve().count(), 1);
    }

    #[test]
    fn generate_validates_n_too_small() {
        let mut r = rng(0);
        assert!(Puzzle::generate(0, &mut r).is_err());
    }

    #[test]
    fn generate_validates_n_too_large() {
        let mut r = rng(0);
        assert!(Puzzle::generate(10, &mut r).is_err());
    }

    #[test]
    fn generate_with_routes_through_the_supplied_op_policy() {
        // A custom op policy that counts how many times it's called and delegates
        // to default_op_policy. The generated puzzle must have at least one cage,
        // so the policy must run at least once.
        let mut r = rng(42);
        let calls = std::cell::Cell::new(0_u32);
        let op = |values: &[u8], n: usize| {
            calls.set(calls.get() + 1);
            Puzzle::default_op_policy(values, n)
        };
        let puzzle =
            Puzzle::generate_with(4, &mut r, op, SizeDistribution::new(1.0).unwrap()).unwrap();
        assert!(calls.get() > 0);
        assert!(puzzle.cages().count() > 0);
    }

    #[test]
    fn puzzle_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Puzzle>();
    }

    // The tests below mirror in-crate unit tests for the same methods. The
    // duplication is intentional: this file runs as an external consumer would,
    // so it catches accidental visibility regressions that internal tests
    // cannot. Do not consolidate.

    #[test]
    fn polyomino_cells_iterates_in_row_major_order() {
        let p = Polyomino::from_cells(&[cell(1, 0), cell(0, 0), cell(0, 1)]).unwrap();
        let got: Vec<Cell> = p.cells().collect();
        assert_eq!(got, vec![cell(0, 0), cell(0, 1), cell(1, 0)]);
    }

    #[test]
    fn polyomino_len_matches_construction_count() {
        let p = Polyomino::from_cells(&[cell(0, 0), cell(0, 1), cell(0, 2)]).unwrap();
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn polyomino_contains_known_and_unknown_cells() {
        let p = Polyomino::from_cells(&[cell(0, 0), cell(0, 1)]).unwrap();
        assert!(p.contains(cell(0, 0)));
        assert!(p.contains(cell(0, 1)));
        assert!(!p.contains(cell(1, 0)));
    }

    #[test]
    fn is_edge_connected_component_accepts_connected_inputs() {
        assert!(Polyomino::is_edge_connected_component(&[cell(0, 0)]));
        assert!(Polyomino::is_edge_connected_component(&[
            cell(0, 0),
            cell(0, 1)
        ]));
    }

    #[test]
    fn is_edge_connected_component_rejects_disconnected_inputs() {
        assert!(!Polyomino::is_edge_connected_component(&[
            cell(0, 0),
            cell(1, 1)
        ]));
    }

    #[test]
    fn is_edge_connected_component_treats_empty_input_as_connected() {
        assert!(Polyomino::is_edge_connected_component(&[]));
    }

    #[test]
    fn cage_cells_matches_underlying_polyomino() {
        let cells = [cell(0, 0), cell(0, 1)];
        let p = Polyomino::from_cells(&cells).unwrap();
        let cage = Cage::new(4, p.clone(), Operation::Add(3));
        let cage_cells: Vec<Cell> = cage.cells().collect();
        let poly_cells: Vec<Cell> = p.cells().collect();
        assert_eq!(cage_cells, poly_cells);
    }

    #[test]
    fn cage_len_matches_polyomino_len() {
        let p = Polyomino::from_cells(&[cell(0, 0), cell(0, 1), cell(1, 0)]).unwrap();
        let cage = Cage::new(4, p, Operation::Add(6));
        assert_eq!(cage.len(), 3);
    }

    #[test]
    fn candidates_exposes_cell_fill_pairs_via_public_api() {
        // Given(2) at (0,0) pins that cell to {2}; the remaining cells in row 0
        // and column 0 narrow to {1, 3, 4} via AllDifferent.
        let puzzle = Puzzle::with_cages(4, &[cage(4, &[(0, 0)], Operation::Given(2))])
            .unwrap()
            .unwrap();
        let pairs: Vec<(Cell, Fill)> = puzzle.candidates().collect();
        assert_eq!(pairs.len(), 16);
        let pinned = pairs
            .iter()
            .find(|(c, _)| *c == cell(0, 0))
            .map(|(_, f)| *f)
            .unwrap();
        assert_eq!(pinned, Fill::new([2]));
    }
}
