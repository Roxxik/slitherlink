use crate::edge::EdgeState;

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

    pub fn h_edge(&self, x: usize, y: usize) -> EdgeState {
        self.h_edges[self.h_index(x, y)]
    }

    pub fn v_edge(&self, x: usize, y: usize) -> EdgeState {
        self.v_edges[self.v_index(x, y)]
    }

    pub fn set_h_edge(&mut self, x: usize, y: usize, state: EdgeState) {
        let i = self.h_index(x, y);
        self.h_edges[i] = state;
    }

    pub fn set_v_edge(&mut self, x: usize, y: usize, state: EdgeState) {
        let i = self.v_index(x, y);
        self.v_edges[i] = state;
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
