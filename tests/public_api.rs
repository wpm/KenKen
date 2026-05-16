//! Integration tests for the crate's public API. These run as an external user
//! would, guarding against accidental visibility regressions.

#![allow(clippy::unwrap_used)]
mod test {

    use kenken::{
        Cell, Fill, Grid, Operation, Operator, Polyomino, Puzzle, constraints::Cover, generate,
    };
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
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
        let subtract: Vec<Operation> = Polyomino::valid_operations(&cells, Operator::Subtract, 4)
            .unwrap()
            .collect();
        assert_eq!(
            subtract,
            vec![
                Operation::Subtract(1),
                Operation::Subtract(2),
                Operation::Subtract(3),
            ]
        );
        let divide: Vec<Operation> = Polyomino::valid_operations(&cells, Operator::Divide, 4)
            .unwrap()
            .collect();
        assert_eq!(
            divide,
            vec![
                Operation::Divide(2),
                Operation::Divide(3),
                Operation::Divide(4),
            ]
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
