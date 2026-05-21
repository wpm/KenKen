//! A custom Pumpkin [`Propagator`] enforcing all-different over a row or column
//! by wrapping the crate's generalized [`regin`] arc-consistency filter.
//!
//! Pumpkin's built-in `all_different` is a pairwise `!=` decomposition, which
//! misses Hall sets. This propagator instead snapshots the line's domains, runs
//! [`regin`] for full GAC, and posts every value it removes. Doing so makes the
//! generalized Régin from #97 the runtime all-different engine, replacing the
//! built-in decomposition used in step 2.
//!
//! ## Explanations
//!
//! Each removal is justified by the snapshot domain state: the conjunction of
//! every `var != w` that already holds across the line (a value `w` removed
//! from some cell before this call). Régin computed the pruning from exactly
//! that state, so it entails the removal under all-different. The reason is
//! coarser than a minimal Hall-set certificate but always sound; tightening it
//! is a possible later optimization.

use std::rc::Rc;

use pumpkin_solver::core::{
    convert_case, declare_inference_label, predicate,
    predicates::{Predicate, PropositionalConjunction},
    proof::{ConstraintTag, InferenceCode},
    propagation::{
        DomainEvents, LocalId, PropagationContext, Propagator, PropagatorConstructor,
        PropagatorConstructorContext, ReadDomains,
    },
    results::PropagationStatusCP,
    variables::DomainId,
};

use crate::{Fill, constraints::regin::regin, engine::value_of, types::N};

declare_inference_label!(ReginInference);

/// Constructor (the "args" half) for [`ReginPropagator`]. Registers a watch on
/// every line variable.
#[derive(Clone, Debug)]
pub struct ReginArgs {
    /// The row or column's cell variables.
    pub vars: Rc<[DomainId]>,
    /// Grid size; every variable's initial domain is `1..=n`.
    pub n: N,
    /// Tag grouping this line's inferences in the proof log.
    pub tag: ConstraintTag,
}

impl PropagatorConstructor for ReginArgs {
    type PropagatorImpl = ReginPropagator;

    fn create(self, mut context: PropagatorConstructorContext) -> Self::PropagatorImpl {
        for (i, &var) in self.vars.iter().enumerate() {
            context.register(
                var,
                DomainEvents::ANY_INT,
                LocalId::from(u32::try_from(i).unwrap_or(u32::MAX)),
            );
        }
        ReginPropagator {
            code: InferenceCode::new(self.tag, ReginInference),
            vars: self.vars,
            n: self.n,
        }
    }
}

/// GAC all-different propagator for one line, backed by [`regin`].
#[derive(Clone, Debug)]
pub struct ReginPropagator {
    vars: Rc<[DomainId]>,
    n: N,
    code: InferenceCode,
}

impl ReginPropagator {
    /// Reads the current domain of each variable into a [`Fill`] snapshot.
    fn snapshot(&self, context: &PropagationContext) -> Vec<Fill> {
        self.vars
            .iter()
            .map(|var| context.iterate_domain(var).map(value_of).collect())
            .collect()
    }

    /// The sound explanation shared by every removal this call makes: every
    /// `var != w` already true across the line at snapshot time.
    fn state_reason(&self, snapshot: &[Fill]) -> PropositionalConjunction {
        let mut predicates: Vec<Predicate> = Vec::new();
        for (&var, fill) in self.vars.iter().zip(snapshot) {
            for w in 1..=self.n {
                if (*fill & Fill::new([w])).is_empty() {
                    let absent = i32::from(w);
                    predicates.push(predicate![var != absent]);
                }
            }
        }
        predicates.into_iter().collect()
    }
}

impl Propagator for ReginPropagator {
    #[allow(clippy::unnecessary_literal_bound)] // trait fixes the signature to `-> &str`
    fn name(&self) -> &str {
        "Regin"
    }

    fn propagate_from_scratch(&self, mut context: PropagationContext) -> PropagationStatusCP {
        let snapshot = self.snapshot(&context);
        let pruned = regin(&snapshot);
        let reason = self.state_reason(&snapshot);
        for ((&var, before), after) in self.vars.iter().zip(&snapshot).zip(&pruned) {
            for value in before.iter() {
                if (*after & Fill::new([value])).is_empty() {
                    let removed = i32::from(value);
                    context.post(predicate![var != removed], reason.clone(), &self.code)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, deprecated)]
mod tests {
    use pumpkin_solver::{Solver, core::TestSolver};

    use super::*;

    #[test]
    fn name_is_regin() {
        let mut solver = Solver::default();
        let tag = solver.new_constraint_tag();
        let vars: Rc<[DomainId]> = Vec::new().into();
        let propagator = ReginPropagator {
            vars,
            n: 4,
            code: InferenceCode::unknown_label(tag),
        };
        assert_eq!(propagator.name(), "Regin");
    }

    #[test]
    fn prunes_hall_set_on_4x4_line() {
        // Two of four cells in a 4-line are pinned to {1,2}. That Hall set
        // saturates values 1 and 2, so the generalized Régin propagator must
        // remove both from the other two cells — a prune pairwise `!=` misses.
        let mut test = TestSolver::default();
        let a = test.new_variable(1, 4);
        let b = test.new_variable(1, 4);
        let c = test.new_variable(1, 4);
        let d = test.new_variable(1, 4);
        for var in [a, b] {
            test.remove(var, 3).unwrap();
            test.remove(var, 4).unwrap();
        }
        let tag = test.new_constraint_tag();
        let vars: Rc<[DomainId]> = [a, b, c, d].into();
        let _ = test.new_propagator(ReginArgs { vars, n: 4, tag }).unwrap();
        for var in [c, d] {
            assert!(!test.contains(var, 1));
            assert!(!test.contains(var, 2));
            assert!(test.contains(var, 3));
            assert!(test.contains(var, 4));
        }
    }

    #[test]
    fn detects_pigeonhole_conflict() {
        // Three cells confined to two values has no all-different assignment;
        // the propagator must report a conflict.
        let mut test = TestSolver::default();
        let a = test.new_variable(1, 3);
        let b = test.new_variable(1, 3);
        let c = test.new_variable(1, 3);
        for var in [a, b, c] {
            test.remove(var, 3).unwrap();
        }
        let tag = test.new_constraint_tag();
        let vars: Rc<[DomainId]> = [a, b, c].into();
        assert!(test.new_propagator(ReginArgs { vars, n: 3, tag }).is_err());
    }
}
