use crate::edge::{EdgeId, EdgeState};

/// Edge state for a puzzle of size `width` x `height`.
///
/// Horizontal edges live on `height + 1` rows of `width` edges each.
/// Vertical edges live on `height` rows of `width + 1` edges each.
///
/// Coordinates: `h_edge(x, y)` connects vertex (x, y) to (x+1, y); `v_edge(x, y)` connects
/// vertex (x, y) to (x, y+1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Solution {
    width: usize,
    height: usize,
    h_edges: Vec<EdgeState>,
    v_edges: Vec<EdgeState>,
}

impl Solution {
    pub fn empty(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            h_edges: vec![EdgeState::Unset; width * (height + 1)],
            v_edges: vec![EdgeState::Unset; (width + 1) * height],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Reads any edge by id. The axis-specific [`h_edge`](Self::h_edge) and
    /// [`v_edge`](Self::v_edge) helpers below are thin wrappers over this.
    #[inline]
    pub fn edge(&self, e: EdgeId) -> EdgeState {
        match e {
            EdgeId::H(x, y) => {
                let i = self.h_index(x, y);
                self.h_edges[i]
            }
            EdgeId::V(x, y) => {
                let i = self.v_index(x, y);
                self.v_edges[i]
            }
        }
    }

    /// The single mutation site for edge state. Future bookkeeping (incremental
    /// loop components, dirty-list tracking) hooks in here so it covers all
    /// callers automatically.
    #[inline]
    pub fn set_edge(&mut self, e: EdgeId, state: EdgeState) {
        match e {
            EdgeId::H(x, y) => {
                let i = self.h_index(x, y);
                self.h_edges[i] = state;
            }
            EdgeId::V(x, y) => {
                let i = self.v_index(x, y);
                self.v_edges[i] = state;
            }
        }
    }

    #[inline]
    pub fn h_edge(&self, x: usize, y: usize) -> EdgeState {
        self.edge(EdgeId::H(x, y))
    }

    #[inline]
    pub fn v_edge(&self, x: usize, y: usize) -> EdgeState {
        self.edge(EdgeId::V(x, y))
    }

    #[inline]
    pub fn set_h_edge(&mut self, x: usize, y: usize, state: EdgeState) {
        self.set_edge(EdgeId::H(x, y), state);
    }

    #[inline]
    pub fn set_v_edge(&mut self, x: usize, y: usize, state: EdgeState) {
        self.set_edge(EdgeId::V(x, y), state);
    }

    fn h_index(&self, x: usize, y: usize) -> usize {
        assert!(x < self.width && y <= self.height, "h_edge ({x},{y}) out of bounds for {}x{}", self.width, self.height);
        y * self.width + x
    }

    fn v_index(&self, x: usize, y: usize) -> usize {
        assert!(x <= self.width && y < self.height, "v_edge ({x},{y}) out of bounds for {}x{}", self.width, self.height);
        y * (self.width + 1) + x
    }
}
