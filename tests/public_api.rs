//! Integration tests for the crate's public API. These run as an external user
//! would, guarding against accidental visibility regressions.

#![allow(clippy::unwrap_used)]
mod test {

    use kenken::{
        Cage, Cell, Fill, Grid, Operation, Operator, Polyomino, Puzzle, Solver, constraints::Cover,
        generate,
    };
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
    }

    /// Builds a [`Cage`] over `cells` (given as `(row, column)` pairs) for an
    /// `n`-sized grid.
    fn cage(n: u8, cells: &[(usize, usize)], op: Operation) -> Cage {
        let cells: Vec<Cell> = cells.iter().map(|&(r, c)| Cell::new(r, c)).collect();
        Cage::new(n, Polyomino::from_cells(&cells).unwrap(), op)
    }

    /// A 6×6 puzzle fully covered by 18 vertical `Subtract(1)` pair cages,
    /// one per column pair `((0,c)-(1,c))`, `((2,c)-(3,c))`, `((4,c)-(5,c))`.
    ///
    /// Each cage admits both orderings of every consecutive pair, so many
    /// Latin-square completions satisfy the constraints.
    fn multi_solution_puzzle() -> Puzzle {
        let grid = Grid::new(6).unwrap();
        let mut cages = Vec::with_capacity(18);
        for col in 0..6 {
            for (r0, r1) in [(0, 1), (2, 3), (4, 5)] {
                cages.push(cage(6, &[(r0, col), (r1, col)], Operation::Subtract(1)));
            }
        }
        Puzzle::new(&grid, &cages).unwrap().unwrap()
    }

    /// The [`multi_solution_puzzle`] layout with one cage swapped from
    /// `Subtract(1)` to `Subtract(4)`. Initial propagation narrows
    /// `(0,0)` and `(1,0)` to `{1, 2, 5, 6}` but cannot see the global
    /// infeasibility: every assignment of column 0 leaves the remaining
    /// four cells as `{2,3,4,6}` or `{1,3,4,5}`, neither of which can be
    /// partitioned into two consecutive-integer pairs for the
    /// `(2,0)-(3,0)` and `(4,0)-(5,0)` `Subtract(1)` cages.
    fn no_solution_puzzle() -> Puzzle {
        let grid = Grid::new(6).unwrap();
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
        Puzzle::new(&grid, &cages).unwrap().unwrap()
    }

    #[test]
    fn solver_finds_multiple_solutions_for_underconstrained_puzzle() {
        let puzzle = multi_solution_puzzle();
        let start = std::time::Instant::now();
        let mut solver = Solver::new(puzzle);
        assert!(solver.next().is_some());
        assert!(solver.next().is_some());
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn solver_finds_no_solutions_for_infeasible_puzzle() {
        let puzzle = no_solution_puzzle();
        let start = std::time::Instant::now();
        let mut solver = Solver::new(puzzle);
        assert!(solver.next().is_none());
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
    }

    #[test]
    fn generate_validates_n_too_small() {
        let mut r = rng(0);
        assert!(generate(0, &mut r).is_err());
    }

    #[test]
    fn generate_validates_n_too_large() {
        let mut r = rng(0);
        assert!(generate(10, &mut r).is_err());
    }

    #[test]
    fn puzzle_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Puzzle>();
    }

    #[test]
    fn grid_set_is_public_and_round_trips() {
        let g: Grid = Grid::new(3).unwrap();
        let cell = Cell::new(1, 2);
        let one = Fill::new([1]);
        let g = g.set(&cell, one);
        assert_eq!(g.get(&cell).unwrap(), one);
    }

    #[test]
    fn polyomino_insert_and_remove_are_public() {
        let p = Polyomino::from_cells(&[Cell::new(0, 0), Cell::new(0, 1)]).unwrap();
        let extended = p.insert(Cell::new(0, 2)).unwrap();
        assert_eq!(extended.len(), 3);
        let shrunk = extended.remove(Cell::new(0, 2)).unwrap();
        assert_eq!(shrunk, p);
    }

    #[test]
    fn cage_valid_operators_branches_by_cell_count() {
        assert_eq!(
            Polyomino::valid_operators(&[Cell::new(0, 0)]),
            vec![Operator::Given]
        );
        assert_eq!(
            Polyomino::valid_operators(&[Cell::new(0, 0), Cell::new(0, 1)]),
            vec![
                Operator::Add,
                Operator::Subtract,
                Operator::Multiply,
                Operator::Divide,
            ]
        );
        assert_eq!(
            Polyomino::valid_operators(&[Cell::new(0, 0), Cell::new(0, 1), Cell::new(0, 2)]),
            vec![Operator::Add, Operator::Multiply]
        );
    }

    #[test]
    fn cage_valid_targets_yields_legal_targets_in_ascending_order() {
        let cells = [Cell::new(0, 0), Cell::new(0, 1)];
        itertools::assert_equal(
            Polyomino::valid_operations(&cells, Operator::Subtract, 4).unwrap(),
            [
                Operation::Subtract(1),
                Operation::Subtract(2),
                Operation::Subtract(3),
            ],
        );
        itertools::assert_equal(
            Polyomino::valid_operations(&cells, Operator::Divide, 4).unwrap(),
            [
                Operation::Divide(2),
                Operation::Divide(3),
                Operation::Divide(4),
            ],
        );
    }

    #[test]
    fn cage_is_valid_filters_operator_target_pairs() {
        let singleton = [Cell::new(0, 0)];
        assert!(Polyomino::is_valid_operation(&singleton, Operation::Given(3), 4).unwrap());
        assert!(!Polyomino::is_valid_operation(&singleton, Operation::Add(3), 4).unwrap());

        let pair = [Cell::new(0, 0), Cell::new(0, 1)];
        assert!(Polyomino::is_valid_operation(&pair, Operation::Subtract(2), 4).unwrap());
        assert!(!Polyomino::is_valid_operation(&pair, Operation::Subtract(0), 4).unwrap());
        assert!(!Polyomino::is_valid_operation(&pair, Operation::Divide(1), 4).unwrap());

        let triple = [Cell::new(0, 0), Cell::new(0, 1), Cell::new(0, 2)];
        assert!(Polyomino::is_valid_operation(&triple, Operation::Add(6), 4).unwrap());
        assert!(!Polyomino::is_valid_operation(&triple, Operation::Subtract(1), 4).unwrap());
    }
}
