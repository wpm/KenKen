use std::cmp::Ordering;

use crate::{Cage, Cell, Operation, Polyomino, constraints::Cover, types::N};

/// A slot in a puzzle: either a claimed [`Polyomino`] region with no operation
/// yet (`Region`), or a fully specified [`Cage`].
///
/// `CageSlot` lets the library model incomplete puzzles directly, so the
/// Designer can promote a `Region` to a `Cage` (and demote it back) without
/// reaching for a parallel draft type.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CageSlot {
    Region(Polyomino),
    Cage(Cage),
}

impl CageSlot {
    /// Returns the polyomino covered by this slot, regardless of variant.
    pub const fn polyomino(&self) -> &Polyomino {
        match self {
            Self::Region(p) => p,
            Self::Cage(c) => c.polyomino(),
        }
    }

    /// Returns the inner [`Cage`] for the `Cage` variant, or `None` otherwise.
    pub const fn as_cage(&self) -> Option<&Cage> {
        if let Self::Cage(c) = self {
            Some(c)
        } else {
            None
        }
    }

    /// Returns the inner [`Polyomino`] for the `Region` variant, or `None`
    /// otherwise.
    pub const fn as_region(&self) -> Option<&Polyomino> {
        if let Self::Region(p) = self {
            Some(p)
        } else {
            None
        }
    }
}

impl Cover for CageSlot {
    fn cells(&self) -> impl Iterator<Item = Cell> {
        self.polyomino().cells()
    }
}

// `Ord` and `Eq` deliberately disagree: `Region(p)` and `Cage(c)` with the
// same polyomino compare as `Ordering::Equal` under `cmp` but are NOT `==`.
// This keeps the Designer's tab order stable across promote/demote.
impl Ord for CageSlot {
    fn cmp(&self, other: &Self) -> Ordering {
        self.polyomino().cmp(other.polyomino())
    }
}

impl PartialOrd for CageSlot {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl serde::Serialize for CageSlot {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStructVariant;
        match self {
            Self::Region(p) => s.serialize_newtype_variant("CageSlot", 0, "Region", p),
            Self::Cage(c) => {
                let mut sv = s.serialize_struct_variant("CageSlot", 1, "Cage", 3)?;
                sv.serialize_field("polyomino", c.polyomino())?;
                sv.serialize_field("operation", &c.operation())?;
                sv.serialize_field("n", &c.n())?;
                sv.end()
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for CageSlot {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        enum Wire {
            Region(Polyomino),
            Cage {
                polyomino: Polyomino,
                operation: Operation,
                n: N,
            },
        }
        Ok(match Wire::deserialize(d)? {
            Wire::Region(p) => Self::Region(p),
            Wire::Cage {
                polyomino,
                operation,
                n,
            } => Self::Cage(Cage::new(n, polyomino, operation)),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::constraints::test_utils::{pair, singleton};

    fn region_singleton() -> CageSlot {
        CageSlot::Region(singleton())
    }

    fn cage_singleton() -> CageSlot {
        CageSlot::Cage(Cage::new(4, singleton(), Operation::Given(1)))
    }

    // --- Accessors ---

    #[test]
    fn as_cage_returns_some_for_cage_variant_and_none_for_region() {
        let cage = Cage::new(4, singleton(), Operation::Given(3));
        let slot = CageSlot::Cage(cage.clone());
        assert_eq!(slot.as_cage(), Some(&cage));
        assert_eq!(region_singleton().as_cage(), None);
    }

    #[test]
    fn as_region_returns_some_for_region_variant_and_none_for_cage() {
        let p = singleton();
        let slot = CageSlot::Region(p.clone());
        assert_eq!(slot.as_region(), Some(&p));
        assert_eq!(cage_singleton().as_region(), None);
    }

    #[test]
    fn polyomino_returns_inner_polyomino_for_both_variants() {
        let p = pair();
        assert_eq!(CageSlot::Region(p.clone()).polyomino(), &p);
        let cage = Cage::new(4, p.clone(), Operation::Add(6));
        assert_eq!(CageSlot::Cage(cage).polyomino(), &p);
    }

    // --- Cover ---

    #[test]
    fn cover_cells_match_polyomino_cells_for_both_variants() {
        let p = pair();
        let expected: Vec<Cell> = p.cells().collect();
        let region = CageSlot::Region(p.clone());
        let cage_slot = CageSlot::Cage(Cage::new(4, p, Operation::Add(6)));
        assert_eq!(region.cells().collect::<Vec<_>>(), expected);
        assert_eq!(cage_slot.cells().collect::<Vec<_>>(), expected);
    }

    // --- Ord / PartialOrd ---

    #[test]
    fn cmp_equal_across_variants_with_same_polyomino() {
        let region = CageSlot::Region(singleton());
        let cage = CageSlot::Cage(Cage::new(4, singleton(), Operation::Given(1)));
        // Tab order is stable across promote/demote.
        assert_eq!(region.cmp(&cage), Ordering::Equal);
        // But the variants are not value-equal: documents the intentional
        // Ord/Eq divergence.
        assert_ne!(region, cage);
    }

    #[test]
    fn cmp_orders_by_polyomino_ignoring_variant() {
        let region_small = CageSlot::Region(singleton());
        let cage_large = CageSlot::Cage(Cage::new(4, pair(), Operation::Add(3)));
        assert!(region_small < cage_large);

        let cage_small = CageSlot::Cage(Cage::new(4, singleton(), Operation::Given(1)));
        let region_large = CageSlot::Region(pair());
        assert!(cage_small < region_large);
    }

    #[test]
    fn partial_cmp_consistent_with_cmp() {
        let a = region_singleton();
        let b = CageSlot::Region(pair());
        assert_eq!(a.partial_cmp(&b), Some(a.cmp(&b)));
    }

    // --- Serde round-trip ---

    #[test]
    fn region_round_trips_through_json() {
        let original = CageSlot::Region(pair());
        let json = serde_json::to_string(&original).unwrap();
        let restored: CageSlot = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn cage_round_trips_through_json() {
        let cage = Cage::new(4, pair(), Operation::Add(6));
        let original = CageSlot::Cage(cage);
        let json = serde_json::to_string(&original).unwrap();
        let restored: CageSlot = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
        assert_eq!(
            restored.as_cage().unwrap().tuples(),
            original.as_cage().unwrap().tuples(),
        );
    }

    #[test]
    fn cage_deserialize_missing_n_returns_err() {
        // `n` is mandatory on the wire for the Cage variant.
        let json = r#"{"Cage":{"polyomino":[{"row":0,"column":0}],"operation":{"Given":3}}}"#;
        assert!(serde_json::from_str::<CageSlot>(json).is_err());
    }
}
