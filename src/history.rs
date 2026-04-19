use crate::types::{Cage, Cell, Tuple, Value};

/// The domain state of a puzzle: maps each cell to its remaining possible values.
/// An empty set for any cell means that cell has failed (no valid assignment).
/// A singleton set means the cell is fully determined.
///
/// Uses BTreeMap for deterministic iteration order (required for reproducible MRV branching).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DomainState {
    pub cell_domains: std::collections::BTreeMap<Cell, std::collections::BTreeSet<Value>>,
}

impl DomainState {
    pub fn is_solved(&self) -> bool {
        !self.cell_domains.is_empty() && self.cell_domains.values().all(|d| d.len() == 1)
    }

    pub fn is_failed(&self) -> bool {
        self.cell_domains.values().any(|d| d.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Variable {
    Cell(Cell),
    Cage(Cage),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Assignment {
    Value(Value),
    Tuple(Tuple),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    CellValueRemoved {
        cell: Cell,
        value: Value,
    },
    TupleEliminated {
        cage: Cage,
        tuple: Tuple,
    },
    BranchPoint {
        variable: Variable,
        value: Assignment,
    },
    MergeAttempted {
        cage_a: Cage,
        cage_b: Cage,
        accepted: bool,
    },
    SplitPerformed {
        cage: Cage,
        result_a: Cage,
        result_b: Cage,
    },
    CounterexampleFound {
        sol: DomainState,
    },
}

pub type History = Vec<Event>;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HistorySummary {
    pub cell_value_removed: usize,
    pub tuple_eliminated: usize,
    pub branch_points: usize,
    pub merge_attempted: usize,
    pub merge_accepted: usize,
    pub split_performed: usize,
    pub counterexamples_found: usize,
}

impl HistorySummary {
    pub fn from_history(h: &History) -> Self {
        h.iter().fold(HistorySummary::default(), |mut acc, event| {
            match event {
                Event::CellValueRemoved { .. } => acc.cell_value_removed += 1,
                Event::TupleEliminated { .. } => acc.tuple_eliminated += 1,
                Event::BranchPoint { .. } => acc.branch_points += 1,
                Event::MergeAttempted { accepted, .. } => {
                    acc.merge_attempted += 1;
                    if *accepted {
                        acc.merge_accepted += 1;
                    }
                }
                Event::SplitPerformed { .. } => acc.split_performed += 1,
                Event::CounterexampleFound { .. } => acc.counterexamples_found += 1,
            }
            acc
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SolveResult {
    Unique(DomainState),
    NoSolution,
    NonUnique(DomainState, DomainState),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::fixtures::make_cage;
    use crate::types::Operation;

    #[test]
    fn history_summary_counts() {
        let cage_a = make_cage(vec![(0, 0)], Operation::Given(1));
        let cage_b = make_cage(vec![(0, 1)], Operation::Given(1));
        let cage_c = make_cage(vec![(1, 0)], Operation::Given(1));

        let history: History = vec![
            // 2 CellValueRemoved
            Event::CellValueRemoved {
                cell: (0, 0),
                value: 1,
            },
            Event::CellValueRemoved {
                cell: (0, 1),
                value: 2,
            },
            // 1 TupleEliminated
            Event::TupleEliminated {
                cage: cage_a.clone(),
                tuple: vec![1, 2],
            },
            // 3 BranchPoint
            Event::BranchPoint {
                variable: Variable::Cell((1, 1)),
                value: Assignment::Value(3),
            },
            Event::BranchPoint {
                variable: Variable::Cage(cage_a.clone()),
                value: Assignment::Tuple(vec![1, 2]),
            },
            Event::BranchPoint {
                variable: Variable::Cell((2, 2)),
                value: Assignment::Value(1),
            },
            // 2 MergeAttempted: 1 accepted=true, 1 accepted=false
            Event::MergeAttempted {
                cage_a: cage_a.clone(),
                cage_b: cage_b.clone(),
                accepted: true,
            },
            Event::MergeAttempted {
                cage_a: cage_b.clone(),
                cage_b: cage_c.clone(),
                accepted: false,
            },
            // 1 SplitPerformed
            Event::SplitPerformed {
                cage: cage_a.clone(),
                result_a: cage_b.clone(),
                result_b: cage_c.clone(),
            },
        ];

        let summary = HistorySummary::from_history(&history);

        assert_eq!(summary.cell_value_removed, 2);
        assert_eq!(summary.tuple_eliminated, 1);
        assert_eq!(summary.branch_points, 3);
        assert_eq!(summary.merge_attempted, 2);
        assert_eq!(summary.merge_accepted, 1);
        assert_eq!(summary.split_performed, 1);
        assert_eq!(summary.counterexamples_found, 0);
    }

    #[test]
    fn solve_result_variants_match() {
        let unique = SolveResult::Unique(DomainState::default());
        let no_sol = SolveResult::NoSolution;
        let non_unique = SolveResult::NonUnique(DomainState::default(), DomainState::default());

        assert!(matches!(unique, SolveResult::Unique(_)));
        assert!(matches!(no_sol, SolveResult::NoSolution));
        assert!(matches!(non_unique, SolveResult::NonUnique(_, _)));
    }

    #[test]
    fn empty_history_summary() {
        let history: History = vec![];
        let summary = HistorySummary::from_history(&history);

        assert_eq!(summary.cell_value_removed, 0);
        assert_eq!(summary.tuple_eliminated, 0);
        assert_eq!(summary.branch_points, 0);
        assert_eq!(summary.merge_attempted, 0);
        assert_eq!(summary.merge_accepted, 0);
        assert_eq!(summary.split_performed, 0);
        assert_eq!(summary.counterexamples_found, 0);
    }
}
