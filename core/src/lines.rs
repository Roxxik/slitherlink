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

    /// True iff every loop edge belongs to a single closed cycle.
    ///
    /// Precondition: a degree-valid board (every vertex has loop-degree 0 or 2).
    /// Callers reach this only after a degree check; without it a single open
    /// path would also report true. The default recomputes by counting edges and
    /// walking one cycle; [`SolverLines`](crate::SolverLines) overrides it with an
    /// O(1) read of its incremental topology.
    fn is_single_loop(&self) -> bool {
        let w = self.width();
        let h = self.height();
        let total = total_loop_edges(self, w, h);
        if total == 0 {
            return false;
        }
        let Some(start) = first_loop_vertex(self, w, h) else {
            return false;
        };
        walk_cycle(self, start, w, h) == total
    }
}

fn total_loop_edges<L: Lines + ?Sized>(s: &L, w: usize, h: usize) -> usize {
    let mut n = 0;
    for y in 0..=h {
        for x in 0..w {
            if s.h_edge(x, y) == EdgeState::Loop {
                n += 1;
            }
        }
    }
    for y in 0..h {
        for x in 0..=w {
            if s.v_edge(x, y) == EdgeState::Loop {
                n += 1;
            }
        }
    }
    n
}

fn first_loop_vertex<L: Lines + ?Sized>(s: &L, w: usize, h: usize) -> Option<(usize, usize)> {
    for y in 0..=h {
        for x in 0..=w {
            if loop_neighbors_at(s, x, y, w, h).next().is_some() {
                return Some((x, y));
            }
        }
    }
    None
}

/// Walks the cycle from `start`, counting edges until it returns or dead-ends.
/// Assumes every vertex on the path has loop-degree exactly 2.
fn walk_cycle<L: Lines + ?Sized>(s: &L, start: (usize, usize), w: usize, h: usize) -> usize {
    let mut prev: Option<(usize, usize)> = None;
    let mut current = start;
    let mut count = 0;
    loop {
        let next = loop_neighbors_at(s, current.0, current.1, w, h).find(|&n| Some(n) != prev);
        let Some(next) = next else {
            return count;
        };
        prev = Some(current);
        current = next;
        count += 1;
        if current == start {
            return count;
        }
    }
}

fn loop_neighbors_at<L: Lines + ?Sized>(s: &L, x: usize, y: usize, w: usize, h: usize) -> impl Iterator<Item = (usize, usize)> {
    [
        (x > 0 && s.h_edge(x - 1, y) == EdgeState::Loop).then(|| (x - 1, y)),
        (x < w && s.h_edge(x, y) == EdgeState::Loop).then(|| (x + 1, y)),
        (y > 0 && s.v_edge(x, y - 1) == EdgeState::Loop).then(|| (x, y - 1)),
        (y < h && s.v_edge(x, y) == EdgeState::Loop).then(|| (x, y + 1)),
    ]
    .into_iter()
    .flatten()
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
