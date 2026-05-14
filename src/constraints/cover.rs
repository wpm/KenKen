use crate::{Cell, Error};

/// A set of [`Cell`]s.
pub trait Cover {
    /// This object's [`Cell`]s in row-major order.
    fn cells(&self) -> Vec<Cell>;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// An arbitrary contiguous region of edge-connected [`Cell`]s.
#[must_use]
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Polyomino(Vec<Cell>);

impl Polyomino {
    /// # Errors
    /// Returns [`Error::EmptyPolyomino`] if `cells` is empty, or
    /// [`Error::DisconnectedPolyomino`] if `cells` is not edge-connected.
    pub fn new(cells: &[Cell]) -> Result<Self, Error> {
        if cells.is_empty() {
            return Err(Error::EmptyPolyomino);
        }
        if !is_edge_connected_component(cells) {
            return Err(Error::DisconnectedPolyomino);
        }
        Ok(Self(polyomino_cell_order_storage(cells)))
    }

    /// Builds a polyomino without validating non-emptiness or connectivity.
    ///
    /// Callers must guarantee `cells` is non-empty and edge-connected.
    pub(crate) fn new_unchecked(cells: &[Cell]) -> Self {
        Self(polyomino_cell_order_storage(cells))
    }

    /// Returns a new polyomino with `cell` added.
    ///
    /// Idempotent: if `cell` is already present, returns an equivalent
    /// polyomino.
    ///
    /// # Errors
    /// Returns [`Error::DisconnectedPolyomino`] if adding `cell` would make the
    /// polyomino disconnected.
    pub fn insert(&self, cell: Cell) -> Result<Self, Error> {
        let mut cells = self.0.clone();
        cells.push(cell);
        Self::new(&cells)
    }

    /// Returns a new polyomino with `cell` removed.
    ///
    /// Idempotent: if `cell` is not present, returns an equivalent polyomino.
    ///
    /// # Errors
    /// Returns [`Error::EmptyPolyomino`] if removing `cell` would empty the
    /// polyomino, or [`Error::DisconnectedPolyomino`] if it would leave the
    /// remaining cells disconnected.
    pub fn remove(&self, cell: Cell) -> Result<Self, Error> {
        let cells = self
            .0
            .iter()
            .copied()
            .filter(|c| *c != cell)
            .collect::<Vec<_>>();
        Self::new(&cells)
    }
}

impl Cover for Polyomino {
    fn cells(&self) -> Vec<Cell> {
        self.0.clone()
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

/// Deduplicated `cells` in row-major order.
#[must_use]
fn polyomino_cell_order_storage(cells: &[Cell]) -> Vec<Cell> {
    let mut cells = cells.to_vec();
    cells.sort();
    cells.dedup();
    cells
}

/// Do `cells` form a contiguous edge-connected component?
/// Two `cell`s are edge-connected if they share a common edge.
#[must_use]
pub fn is_edge_connected_component(cells: &[Cell]) -> bool {
    let cell_set: std::collections::HashSet<Cell> = cells.iter().copied().collect();
    let Some(&start) = cells.first() else {
        return true;
    };
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![start];
    while let Some(cell) = stack.pop() {
        if visited.insert(cell) {
            for neighbor in cell.neighbors_4() {
                if cell_set.contains(&neighbor) {
                    stack.push(neighbor);
                }
            }
        }
    }
    visited.len() == cell_set.len()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // --- is_edge_adjacent ---

    #[test]
    fn is_edge_adjacent_empty_is_true() {
        assert!(is_edge_connected_component(&[]));
    }

    #[test]
    fn is_edge_adjacent_single_cell_is_true() {
        assert!(is_edge_connected_component(&[Cell::new(0, 0)]));
    }

    #[test]
    fn is_edge_adjacent_horizontal_pair_is_true() {
        assert!(is_edge_connected_component(&[
            Cell::new(0, 0),
            Cell::new(0, 1)
        ]));
    }

    #[test]
    fn is_edge_adjacent_vertical_pair_is_true() {
        assert!(is_edge_connected_component(&[
            Cell::new(0, 0),
            Cell::new(1, 0)
        ]));
    }

    #[test]
    fn is_edge_adjacent_diagonal_pair_is_false() {
        assert!(!is_edge_connected_component(&[
            Cell::new(0, 0),
            Cell::new(1, 1)
        ]));
    }

    #[test]
    fn is_edge_adjacent_l_shape_is_true() {
        assert!(is_edge_connected_component(&[
            Cell::new(0, 0),
            Cell::new(1, 0),
            Cell::new(1, 1)
        ]));
    }

    #[test]
    fn is_edge_adjacent_disconnected_is_false() {
        assert!(!is_edge_connected_component(&[
            Cell::new(0, 0),
            Cell::new(0, 2)
        ]));
    }

    // --- Region::polyomino ---

    #[test]
    fn polyomino_cells_are_sorted() {
        let r = Polyomino::new(&[Cell::new(1, 0), Cell::new(0, 0), Cell::new(0, 1)]).unwrap();
        assert_eq!(
            r.cells(),
            vec![Cell::new(0, 0), Cell::new(0, 1), Cell::new(1, 0)]
        );
    }

    #[test]
    fn polyomino_len_matches_cell_count() {
        let p = Polyomino::new(&[Cell::new(0, 0), Cell::new(0, 1), Cell::new(1, 1)]).unwrap();
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn polyomino_new_empty_returns_err() {
        assert!(matches!(Polyomino::new(&[]), Err(Error::EmptyPolyomino)));
    }

    #[test]
    fn polyomino_new_disconnected_returns_err() {
        assert!(matches!(
            Polyomino::new(&[Cell::new(0, 0), Cell::new(0, 2)]),
            Err(Error::DisconnectedPolyomino)
        ));
    }

    #[test]
    fn polyomino_new_distinguishes_empty_from_disconnected() {
        // #45: empty and disconnected inputs must report distinct errors.
        let empty = Polyomino::new(&[]);
        let disconnected = Polyomino::new(&[Cell::new(0, 0), Cell::new(1, 1)]);
        assert!(matches!(empty, Err(Error::EmptyPolyomino)));
        assert!(matches!(disconnected, Err(Error::DisconnectedPolyomino)));
    }

    #[test]
    fn polyomino_new_rejects_diagonal_only_inputs() {
        // #46: replaces the deleted diagonal-polyomino tests — a diagonal-only
        // set of cells is not edge-connected and must be rejected.
        assert!(matches!(
            Polyomino::new(&[Cell::new(0, 0), Cell::new(1, 1), Cell::new(2, 2)]),
            Err(Error::DisconnectedPolyomino)
        ));
    }

    // --- Polyomino::insert ---

    #[test]
    fn insert_adds_adjacent_cell() {
        let p = Polyomino::new(&[Cell::new(0, 0)]).unwrap();
        let p2 = p.insert(Cell::new(0, 1)).unwrap();
        assert!(p2.cells().contains(&Cell::new(0, 1)));
        assert_eq!(p2.len(), 2);
    }

    #[test]
    fn insert_is_idempotent() {
        let p = Polyomino::new(&[Cell::new(0, 0), Cell::new(0, 1)]).unwrap();
        let p2 = p.insert(Cell::new(0, 0)).unwrap();
        assert_eq!(p2.len(), 2);
    }

    #[test]
    fn insert_disconnected_returns_err() {
        let p = Polyomino::new(&[Cell::new(0, 0)]).unwrap();
        assert!(matches!(
            p.insert(Cell::new(0, 2)),
            Err(Error::DisconnectedPolyomino)
        ));
    }

    #[test]
    fn insert_result_is_sorted() {
        let p = Polyomino::new(&[Cell::new(0, 1)]).unwrap();
        let p2 = p.insert(Cell::new(0, 0)).unwrap();
        assert_eq!(p2.cells()[0], Cell::new(0, 0));
    }

    // --- Polyomino::remove ---

    #[test]
    fn remove_deletes_cell() {
        let p = Polyomino::new(&[Cell::new(0, 0), Cell::new(0, 1)]).unwrap();
        let p2 = p.remove(Cell::new(0, 1)).unwrap();
        assert!(!p2.cells().contains(&Cell::new(0, 1)));
        assert_eq!(p2.len(), 1);
    }

    #[test]
    fn remove_is_idempotent() {
        let p = Polyomino::new(&[Cell::new(0, 0), Cell::new(0, 1)]).unwrap();
        let p2 = p.remove(Cell::new(1, 1)).unwrap();
        assert_eq!(p2, p);
    }

    #[test]
    fn remove_last_cell_returns_err() {
        let p = Polyomino::new(&[Cell::new(0, 0)]).unwrap();
        assert!(matches!(
            p.remove(Cell::new(0, 0)),
            Err(Error::EmptyPolyomino)
        ));
    }
}
