//! A custom Pumpkin [`Propagator`] enforcing one cage's arithmetic constraint.
//!
//! A cage admits a fixed set of ordered tuples (precomputed by
//! [`Cage::tuples`](crate::Cage::tuples)): every legal assignment of values to
//! the cage's cells, already filtered for the cage's operator/target and for
//! distinctness within shared rows and columns. This propagator enforces
//! "the cells take one of those tuples" by Simple Tabular Reduction (STR):
//!
//! 1. A tuple is *alive* when every one of its values is still in the matching cell's domain.
//! 2. The values supported at a position are the union, over alive tuples, of the value each places
//!    there — accumulated as a [`Fill`] bitmask.
//! 3. Any value still in a cell's domain but absent from its supported set is pruned.
//!
//! When every value at some position loses support the cage is unsatisfiable;
//! the emptied domain surfaces as a conflict through [`PropagationContext::post`].

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

use crate::{Fill, engine::value_of, types::N};

declare_inference_label!(CageInference);

/// Constructor (the "args" half) for [`CagePropagator`]. Registers a watch on
/// every cage variable, then hands ownership of the tuple table to the
/// propagator.
#[derive(Clone, Debug)]
pub struct CageArgs {
    /// Cage cell variables, in cage cell (row-major) order.
    pub vars: Rc<[DomainId]>,
    /// Valid ordered tuples, each parallel in position to `vars`.
    pub tuples: Rc<[Vec<N>]>,
    /// Tag grouping this cage's inferences in the proof log.
    pub tag: ConstraintTag,
}

impl PropagatorConstructor for CageArgs {
    type PropagatorImpl = CagePropagator;

    fn create(self, mut context: PropagatorConstructorContext) -> Self::PropagatorImpl {
        for (i, &var) in self.vars.iter().enumerate() {
            context.register(
                var,
                DomainEvents::ANY_INT,
                LocalId::from(u32::try_from(i).unwrap_or(u32::MAX)),
            );
        }
        CagePropagator {
            code: InferenceCode::new(self.tag, CageInference),
            vars: self.vars,
            tuples: self.tuples,
        }
    }
}

/// STR propagator for a single cage. Stateless across calls: it recomputes
/// support from scratch each time, which is ample for KenKen's small cages.
#[derive(Clone, Debug)]
pub struct CagePropagator {
    vars: Rc<[DomainId]>,
    tuples: Rc<[Vec<N>]>,
    code: InferenceCode,
}

impl CagePropagator {
    /// Builds the eager reason for pruning `value` at `position`: every tuple
    /// that places `value` there is dead, so for each one we cite a single
    /// witness — a value the tuple needs at some other cell that is no longer
    /// in that cell's domain.
    ///
    /// If no tuple ever places `value` at `position` the conjunction is empty,
    /// which is the correct root-level reason: the cage alone forbids it.
    fn removal_reason(
        &self,
        context: &PropagationContext,
        position: usize,
        value: N,
    ) -> PropositionalConjunction {
        self.tuples
            .iter()
            .filter(|tuple| tuple[position] == value)
            .filter_map(|tuple| {
                tuple.iter().enumerate().find_map(|(j, &needed)| {
                    let var = self.vars[j];
                    let needed = i32::from(needed);
                    (!context.contains(&var, needed)).then(|| predicate![var != needed])
                })
            })
            .collect()
    }
}

impl Propagator for CagePropagator {
    #[allow(clippy::unnecessary_literal_bound)] // trait fixes the signature to `-> &str`
    fn name(&self) -> &str {
        "Cage"
    }

    fn propagate_from_scratch(&self, mut context: PropagationContext) -> PropagationStatusCP {
        let mut supported = vec![Fill::default(); self.vars.len()];
        for tuple in self.tuples.iter() {
            let alive = tuple
                .iter()
                .zip(self.vars.iter())
                .all(|(&value, var)| context.contains(var, i32::from(value)));
            if alive {
                for (slot, &value) in supported.iter_mut().zip(tuple.iter()) {
                    *slot = *slot | Fill::new([value]);
                }
            }
        }

        for (position, support) in supported.iter().enumerate() {
            let var = self.vars[position];
            let unsupported: Vec<i32> = context
                .iterate_domain(&var)
                .filter(|&value| (*support & Fill::new([value_of(value)])).is_empty())
                .collect();
            for value in unsupported {
                let reason = self.removal_reason(&context, position, value_of(value));
                context.post(predicate![var != value], reason, &self.code)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use pumpkin_solver::Solver;

    use super::*;

    #[test]
    fn name_is_cage() {
        let mut solver = Solver::default();
        let tag = solver.new_constraint_tag();
        let vars: Rc<[DomainId]> = Vec::new().into();
        let tuples: Rc<[Vec<N>]> = Vec::new().into();
        let propagator = CagePropagator {
            vars,
            tuples,
            code: InferenceCode::unknown_label(tag),
        };
        assert_eq!(propagator.name(), "Cage");
    }
}
