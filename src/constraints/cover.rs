use crate::Cell;

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
    /// # Panics
    /// Panics if `cells` is empty or not edge-connected.
    pub fn new(cells: &[Cell]) -> Self {
        assert!(!cells.is_empty() && is_edge_connected_component(cells));
        Self(polyomino_cell_order_storage(cells))
    }

    /// Returns a new polyomino with `cell` added.
    ///
    /// Idempotent: if `cell` is already present, returns an equivalent
    /// polyomino. # Panics
    /// Panics if adding `cell` would make the polyomino disconnected.
    pub fn insert(&self, cell: Cell) -> Self {
        let mut cells = self.0.clone();
        cells.push(cell);
        Self::new(&cells)
    }

    /// Returns a new polyomino with `cell` removed.
    ///
    /// Idempotent: if `cell` is not present, returns an equivalent polyomino
    /// wrapped in `Some`. # Panics
    /// Panics if adding `cell` would make the polyomino disconnected.
    pub fn remove(&self, cell: Cell) -> Self {
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
        let r = Polyomino::new(&[Cell::new(1, 0), Cell::new(0, 0), Cell::new(0, 1)]);
        assert_eq!(
            r.cells(),
            vec![Cell::new(0, 0), Cell::new(0, 1), Cell::new(1, 0)]
        );
    }

    #[test]
    fn polyomino_len_matches_cell_count() {
        let p = Polyomino::new(&[Cell::new(0, 0), Cell::new(0, 1), Cell::new(1, 1)]);
        assert_eq!(p.len(), 3);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn polyomino_panics_on_empty() {
        let _ = Polyomino::new(&[]);
    }

    #[test]
    #[should_panic(expected = "assertion failed")]
    fn polyomino_panics_on_disconnected() {
        let _ = Polyomino::new(&[Cell::new(0, 0), Cell::new(0, 2)]);
    }

    // --- Polyomino::insert ---

    #[test]
    fn insert_adds_adjacent_cell() {
        let p = Polyomino::new(&[Cell::new(0, 0)]);
        let p2 = p.insert(Cell::new(0, 1));
        assert!(p2.cells().contains(&Cell::new(0, 1)));
        assert_eq!(p2.len(), 2);
    }

    #[test]
    fn insert_is_idempotent() {
        let p = Polyomino::new(&[Cell::new(0, 0), Cell::new(0, 1)]);
        let p2 = p.insert(Cell::new(0, 0));
        assert_eq!(p2.len(), 2);
    }

    #[test]
    fn insert_result_is_sorted() {
        let p = Polyomino::new(&[Cell::new(0, 1)]);
        let p2 = p.insert(Cell::new(0, 0));
        assert_eq!(p2.cells()[0], Cell::new(0, 0));
    }

    // --- Polyomino::remove ---

    #[test]
    fn remove_deletes_cell() {
        let p = Polyomino::new(&[Cell::new(0, 0), Cell::new(0, 1)]);
        let p2 = p.remove(Cell::new(0, 1));
        assert!(!p2.cells().contains(&Cell::new(0, 1)));
        assert_eq!(p2.len(), 1);
    }

    #[test]
    fn remove_is_idempotent() {
        let p = Polyomino::new(&[Cell::new(0, 0), Cell::new(0, 1)]);
        let p2 = p.remove(Cell::new(1, 1));
        assert_eq!(p2, p);
    }
}
