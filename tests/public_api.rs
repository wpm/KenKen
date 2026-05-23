//! Integration tests for the crate's public API. These run as an external user
//! would, guarding against accidental visibility regressions.

#![allow(clippy::unwrap_used)]
mod test {

    use kenken::{Cage, CageOption, Cell, Domain, Operation, Operator, Polyomino, Puzzle, Slot};

    /// Never called. The `where` bound fails compilation if `Puzzle` loses `Send` or `Sync`.
    const fn _assert_puzzle_is_send_sync()
    where
        Puzzle: Send + Sync,
    {
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

    /// Builds a [`Polyomino`] over `cells` (given as `(row, column)` pairs).
    fn region(cells: &[(usize, usize)]) -> Polyomino {
        let cells: Vec<Cell> = cells.iter().map(|&(r, c)| cell(r, c)).collect();
        Polyomino::from_cells(&cells).unwrap()
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

    mod generator {
        use kenken::{Puzzle, SizeDistribution};
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        fn rng(seed: u64) -> ChaCha8Rng {
            ChaCha8Rng::seed_from_u64(seed)
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
    fn polyomino_feasible_options_is_publicly_callable() {
        let p = region(&[(0, 0), (0, 1)]);
        let opts = p.feasible_options(4);
        assert!(opts.iter().any(|o| matches!(o.op, Operator::Add)));
        assert!(opts.iter().all(|o| !o.targets.is_empty()));
        let json = serde_json::to_string(&opts).unwrap();
        let restored: Vec<CageOption> = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, restored);
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
    fn domains_exposes_cell_domain_pairs_via_public_api() {
        // Given(2) at (0,0) pins that cell to {2}; the remaining cells in row 0
        // and column 0 narrow to {1, 3, 4} via AllDifferent.
        let puzzle = Puzzle::with_cages(4, &[cage(4, &[(0, 0)], Operation::Given(2))])
            .unwrap()
            .unwrap();
        let pairs: Vec<(Cell, Domain)> = puzzle.domains().collect();
        assert_eq!(pairs.len(), 16);
        let pinned = pairs
            .iter()
            .find(|(c, _)| *c == cell(0, 0))
            .map(|(_, f)| *f)
            .unwrap();
        assert_eq!(pinned, Domain::new([2]));
    }

    #[test]
    fn insert_region_is_publicly_callable_and_appears_in_regions_and_slots() {
        let p = region(&[(0, 0)]);
        let puzzle = Puzzle::new(4).unwrap().insert_region(p.clone()).unwrap();
        let regions: Vec<&Polyomino> = puzzle.regions().collect();
        assert_eq!(regions, vec![&p]);
        assert_eq!(puzzle.slots().count(), 1);
    }

    #[test]
    fn promote_then_demote_round_trips() {
        let p = region(&[(0, 0)]);
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_region(p.clone())
            .unwrap()
            .promote(&p, Operation::Given(3))
            .unwrap()
            .unwrap();
        assert_eq!(puzzle.cages().count(), 1);
        let widened = puzzle.demote(&p).unwrap();
        assert_eq!(widened.cages().count(), 0);
        assert_eq!(widened.regions().count(), 1);
        assert_eq!(
            widened.domains().find(|(c, _)| *c == cell(0, 0)).unwrap().1,
            Domain::full(4)
        );
    }

    #[test]
    fn promote_infeasible_returns_public_error() {
        // Subtract on a 3-cell L-shape is not a valid operator for the shape.
        let l = region(&[(0, 0), (1, 0), (1, 1)]);
        let puzzle = Puzzle::new(4).unwrap().insert_region(l.clone()).unwrap();
        assert!(matches!(
            puzzle.promote(&l, Operation::Subtract(1)),
            Err(kenken::Error::InfeasibleOperation(
                _,
                Operation::Subtract(1)
            ))
        ));
    }

    #[test]
    fn insert_region_overlap_returns_public_region_conflict() {
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_cage(cage(4, &[(0, 0)], Operation::Given(3)))
            .unwrap()
            .unwrap();
        assert!(matches!(
            puzzle.insert_region(region(&[(0, 0)])),
            Err(kenken::Error::RegionConflict(_))
        ));
    }

    #[test]
    fn slots_iterator_yields_slot_references() {
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_region(region(&[(0, 0)]))
            .unwrap();
        let slots: Vec<&Slot> = puzzle.slots().collect();
        assert_eq!(slots.len(), 1);
        assert!(matches!(slots[0], Slot::Region(_)));
    }

    #[test]
    fn remove_region_removes_present_region() {
        let p = region(&[(0, 0)]);
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_region(p.clone())
            .unwrap()
            .remove_region(&p)
            .unwrap();
        assert_eq!(puzzle.regions().count(), 0);
        assert_eq!(puzzle.slots().count(), 0);
    }

    #[test]
    fn remove_region_absent_is_noop() {
        let puzzle = Puzzle::new(4).unwrap();
        let after = puzzle.remove_region(&region(&[(0, 0)])).unwrap();
        assert_eq!(after.slots().count(), 0);
    }

    #[test]
    fn remove_region_on_cage_polyomino_is_noop() {
        let c = cage(4, &[(0, 0)], Operation::Given(3));
        let puzzle = Puzzle::new(4).unwrap().insert_cage(c).unwrap().unwrap();
        let after = puzzle.remove_region(&region(&[(0, 0)])).unwrap();
        assert_eq!(after.cages().count(), 1);
        assert_eq!(after.regions().count(), 0);
    }

    #[test]
    fn remove_region_preserves_grid_candidates() {
        let p = region(&[(0, 0)]);
        let puzzle = Puzzle::new(4)
            .unwrap()
            .insert_region(p.clone())
            .unwrap()
            .remove_region(&p)
            .unwrap();
        assert_eq!(
            puzzle.domains().find(|(c, _)| *c == cell(0, 0)).unwrap().1,
            Domain::full(4)
        );
    }

    #[test]
    fn remove_region_then_insert_region_round_trips() {
        let p = region(&[(0, 0)]);
        let base = Puzzle::new(4).unwrap().insert_region(p.clone()).unwrap();
        let round_tripped = base
            .remove_region(&p)
            .unwrap()
            .insert_region(p.clone())
            .unwrap();
        let base_regions: Vec<&Polyomino> = base.regions().collect();
        let round_tripped_regions: Vec<&Polyomino> = round_tripped.regions().collect();
        assert_eq!(base_regions, round_tripped_regions);
    }

    // --- insert_cell (public API) ---

    #[test]
    fn insert_cell_adjacent_cell_grows_region() {
        let p = Puzzle::new(4)
            .unwrap()
            .insert_region(region(&[(0, 0)]))
            .unwrap();
        let slot = Slot::Region(region(&[(0, 0)]));
        let new_p = p.insert_cell(cell(0, 1), &slot).unwrap().unwrap();
        assert_eq!(new_p.regions().count(), 1);
        assert!(new_p.regions().next().unwrap().contains(cell(0, 1)));
    }

    #[test]
    fn insert_cell_on_cage_demotes_and_widens() {
        let c = cage(4, &[(0, 0)], Operation::Given(3));
        let p = Puzzle::new(4)
            .unwrap()
            .insert_cage(c.clone())
            .unwrap()
            .unwrap();
        let slot = Slot::Cage(c);
        let new_p = p.insert_cell(cell(0, 1), &slot).unwrap().unwrap();
        assert_eq!(new_p.cages().count(), 0);
        assert_eq!(new_p.regions().count(), 1);
        assert_eq!(
            new_p.domains().find(|(c, _)| *c == cell(0, 0)).unwrap().1,
            Domain::full(4)
        );
    }

    #[test]
    fn insert_cell_non_adjacent_returns_target_not_adjacent() {
        let p = Puzzle::new(4)
            .unwrap()
            .insert_region(region(&[(0, 0)]))
            .unwrap();
        let slot = Slot::Region(region(&[(0, 0)]));
        assert!(matches!(
            p.insert_cell(cell(1, 1), &slot),
            Err(kenken::Error::TargetNotAdjacent)
        ));
    }

    #[test]
    fn insert_cell_slot_not_in_puzzle_returns_err() {
        let slot = Slot::Region(region(&[(0, 0)]));
        assert!(matches!(
            Puzzle::new(4).unwrap().insert_cell(cell(0, 1), &slot),
            Err(kenken::Error::SlotNotInPuzzle(_))
        ));
    }

    #[test]
    fn insert_cell_into_occupied_cell_returns_region_conflict() {
        let p = Puzzle::new(4)
            .unwrap()
            .insert_region(region(&[(0, 0)]))
            .unwrap()
            .insert_region(region(&[(0, 1)]))
            .unwrap();
        let slot = Slot::Region(region(&[(0, 0)]));
        assert!(matches!(
            p.insert_cell(cell(0, 1), &slot),
            Err(kenken::Error::RegionConflict(_))
        ));
    }

    // --- remove_cell (public API) ---

    #[test]
    fn remove_cell_shrinks_region() {
        let p = Puzzle::new(4)
            .unwrap()
            .insert_region(region(&[(0, 0), (0, 1)]))
            .unwrap();
        let new_p = p.remove_cell(cell(0, 1)).unwrap();
        assert_eq!(new_p.regions().count(), 1);
        assert!(!new_p.regions().next().unwrap().contains(cell(0, 1)));
    }

    #[test]
    fn remove_cell_from_cage_demotes_and_widens() {
        let c = cage(4, &[(0, 0), (0, 1)], Operation::Add(3));
        let p = Puzzle::new(4).unwrap().insert_cage(c).unwrap().unwrap();
        let new_p = p.remove_cell(cell(0, 1)).unwrap();
        assert_eq!(new_p.cages().count(), 0);
        assert_eq!(new_p.regions().count(), 1);
    }

    #[test]
    fn remove_cell_singleton_removes_slot() {
        let p = Puzzle::new(4)
            .unwrap()
            .insert_region(region(&[(0, 0)]))
            .unwrap();
        let new_p = p.remove_cell(cell(0, 0)).unwrap();
        assert_eq!(new_p.slots().count(), 0);
    }

    #[test]
    fn remove_cell_not_covered_returns_cell_not_covered() {
        assert!(matches!(
            Puzzle::new(4).unwrap().remove_cell(cell(0, 0)),
            Err(kenken::Error::CellNotCovered(_))
        ));
    }

    #[test]
    fn remove_cell_would_disconnect_returns_err() {
        let p = Puzzle::new(4)
            .unwrap()
            .insert_region(region(&[(0, 0), (0, 1), (0, 2)]))
            .unwrap();
        assert!(matches!(
            p.remove_cell(cell(0, 1)),
            Err(kenken::Error::WouldDisconnect(_))
        ));
    }
}
