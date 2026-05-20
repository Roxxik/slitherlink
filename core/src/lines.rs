use crate::edge::{EdgeId, EdgeState};

/// Shared read/write surface over a grid of edge states, for a puzzle of size
/// `width` x `height`.
///
/// Horizontal edges live on `height + 1` rows of `width` edges each. Vertical
/// edges live on `height` rows of `width + 1` edges each.
///
/// Coordinates: `h_edge(x, y)` connects vertex (x, y) to (x+1, y); `v_edge(x, y)`
/// connects vertex (x, y) to (x, y+1).
///
/// [`PlayLines`] is the plain implementation; [`SolverLines`](crate::SolverLines)
/// layers incremental loop bookkeeping on top for the propagator.
pub trait Lines {
    fn width(&self) -> usize;
    fn height(&self) -> usize;

    /// Reads any edge by id.
    fn edge(&self, e: EdgeId) -> EdgeState;

    /// The single mutation site for edge state. Implementations hook bookkeeping
    /// (incremental loop components, dirty tracking) in here so it covers all
    /// callers automatically.
    fn set_edge(&mut self, e: EdgeId, state: EdgeState);

    #[inline]
    fn h_edge(&self, x: usize, y: usize) -> EdgeState {
        self.edge(EdgeId::H(x, y))
    }

    #[inline]
    fn v_edge(&self, x: usize, y: usize) -> EdgeState {
        self.edge(EdgeId::V(x, y))
    }

    #[inline]
    fn set_h_edge(&mut self, x: usize, y: usize, state: EdgeState) {
        self.set_edge(EdgeId::H(x, y), state);
    }

    #[inline]
    fn set_v_edge(&mut self, x: usize, y: usize, state: EdgeState) {
        self.set_edge(EdgeId::V(x, y), state);
    }
}

/// Plain edge-state grid used during play. Loop queries are recomputed on the
/// fly, so arbitrary edits (including undo) are always safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayLines {
    width: usize,
    height: usize,
    h_edges: Vec<EdgeState>,
    v_edges: Vec<EdgeState>,
}

impl PlayLines {
    pub fn empty(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            h_edges: vec![EdgeState::Unset; width * (height + 1)],
            v_edges: vec![EdgeState::Unset; (width + 1) * height],
        }
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

impl Lines for PlayLines {
    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }

    #[inline]
    fn edge(&self, e: EdgeId) -> EdgeState {
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

    #[inline]
    fn set_edge(&mut self, e: EdgeId, state: EdgeState) {
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
}
