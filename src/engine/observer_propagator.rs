//! A custom Pumpkin [`Propagator`] that observes, rather than narrows, domains.
//!
//! The Designer needs each cell's candidate set at a propagation fixpoint
//! without running a full solve. This propagator never prunes; it watches every
//! cell variable and, each time it fires, snapshots their current domains (read
//! through [`ReadDomains`]) into shared state. Because adding a propagator drives
//! the solver to a fixpoint, the snapshot taken when the observer is registered
//! reflects the root fixpoint — exactly the per-cell domains the Designer shows.
//!
//! The observer also fires during search (at whatever node Pumpkin re-triggers
//! it), so callers that want the root fixpoint should read the snapshot taken at
//! construction time rather than after a solve. [`Engine`](super::Engine) does
//! this by copying the snapshot out once, immediately after construction.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use pumpkin_solver::core::{
    propagation::{
        DomainEvents, LocalId, PropagationContext, Propagator, PropagatorConstructor,
        PropagatorConstructorContext, ReadDomains,
    },
    results::PropagationStatusCP,
    variables::DomainId,
};

use crate::{Cell, Fill, engine::value_of};

/// Per-cell candidate domains at a propagation fixpoint, in row-major order.
pub type DomainMap = BTreeMap<Cell, Fill>;

/// Shared cell the observer writes each domain snapshot into.
pub type DomainSink = Rc<RefCell<DomainMap>>;

/// Constructor (the "args" half) for [`ObserverPropagator`]. Watches every cell
/// variable so the observer re-fires whenever a domain changes.
#[derive(Clone, Debug)]
pub struct ObserverArgs {
    /// Cells parallel to `vars`.
    pub cells: Rc<[Cell]>,
    /// The cell variables to observe.
    pub vars: Rc<[DomainId]>,
    /// Where each snapshot is written.
    pub sink: DomainSink,
}

impl PropagatorConstructor for ObserverArgs {
    type PropagatorImpl = ObserverPropagator;

    fn create(self, mut context: PropagatorConstructorContext) -> Self::PropagatorImpl {
        for (i, &var) in self.vars.iter().enumerate() {
            context.register(
                var,
                DomainEvents::ANY_INT,
                LocalId::from(u32::try_from(i).unwrap_or(u32::MAX)),
            );
        }
        ObserverPropagator {
            cells: self.cells,
            vars: self.vars,
            sink: self.sink,
        }
    }
}

/// A read-only propagator: it records the current per-cell domains and prunes
/// nothing.
#[derive(Clone, Debug)]
pub struct ObserverPropagator {
    cells: Rc<[Cell]>,
    vars: Rc<[DomainId]>,
    sink: DomainSink,
}

impl Propagator for ObserverPropagator {
    #[allow(clippy::unnecessary_literal_bound)] // trait fixes the signature to `-> &str`
    fn name(&self) -> &str {
        "Observer"
    }

    fn propagate_from_scratch(&self, context: PropagationContext) -> PropagationStatusCP {
        let snapshot: DomainMap = self
            .cells
            .iter()
            .zip(self.vars.iter())
            .map(|(&cell, var)| (cell, context.iterate_domain(var).map(value_of).collect()))
            .collect();
        *self.sink.borrow_mut() = snapshot;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_is_observer() {
        let propagator = ObserverPropagator {
            cells: Vec::new().into(),
            vars: Vec::new().into(),
            sink: DomainSink::default(),
        };
        assert_eq!(propagator.name(), "Observer");
    }
}
