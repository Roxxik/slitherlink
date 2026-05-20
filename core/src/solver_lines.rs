use crate::edge::{EdgeId, EdgeState};
use crate::lines::{Lines, PlayLines};
use crate::loop_topology::{DsuTopology, LoopTopology};

/// Edge-state grid used by the propagator. Wraps [`PlayLines`] and layers
/// incremental loop bookkeeping (`T`) on top, so connectivity queries are answered
/// from the maintained structure instead of a per-pass rebuild.
///
/// The topology backing is a type parameter so it can be swapped (e.g. union-find
/// vs. endpoint linking) and benchmarked; `SolverLines` only ever uses the
/// [`LoopTopology`] interface, never a concrete representation.
///
/// Unlike [`PlayLines`], it assumes monotonic edits (`Unset` -> `Loop`/`Excluded`
/// only), which the propagator guarantees; it is not meant for arbitrary play-mode
/// mutation or undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverLines<T: LoopTopology = DsuTopology> {
    lines: PlayLines,
    topo: T,
}

impl<T: LoopTopology> SolverLines<T> {
    pub fn empty(width: usize, height: usize) -> Self {
        Self {
            lines: PlayLines::empty(width, height),
            topo: T::new(vertex_count(width, height)),
        }
    }

    /// Drops the bookkeeping and returns the underlying [`PlayLines`].
    pub fn into_play(self) -> PlayLines {
        self.lines
    }

    /// True iff grid vertices `a` and `b` are connected through loop edges.
    pub fn loop_connected(&self, a: (usize, usize), b: (usize, usize)) -> bool {
        let w = self.lines.width();
        self.topo.connected(vertex_index(a, w), vertex_index(b, w))
    }
}

impl<T: LoopTopology> From<PlayLines> for SolverLines<T> {
    fn from(lines: PlayLines) -> Self {
        let w = lines.width();
        let h = lines.height();
        let mut topo = T::new(vertex_count(w, h));
        for y in 0..=h {
            for x in 0..w {
                if lines.h_edge(x, y) == EdgeState::Loop {
                    add_edge(&mut topo, EdgeId::H(x, y), w);
                }
            }
        }
        for y in 0..h {
            for x in 0..=w {
                if lines.v_edge(x, y) == EdgeState::Loop {
                    add_edge(&mut topo, EdgeId::V(x, y), w);
                }
            }
        }
        Self { lines, topo }
    }
}

impl<T: LoopTopology> Lines for SolverLines<T> {
    #[inline]
    fn width(&self) -> usize {
        self.lines.width()
    }

    #[inline]
    fn height(&self) -> usize {
        self.lines.height()
    }

    #[inline]
    fn edge(&self, e: EdgeId) -> EdgeState {
        self.lines.edge(e)
    }

    #[inline]
    fn set_edge(&mut self, e: EdgeId, state: EdgeState) {
        let prev = self.lines.edge(e);
        debug_assert!(
            !(prev == EdgeState::Loop && state != EdgeState::Loop),
            "SolverLines is monotonic; a Loop edge cannot be cleared",
        );
        self.lines.set_edge(e, state);
        if state == EdgeState::Loop && prev != EdgeState::Loop {
            let w = self.lines.width();
            add_edge(&mut self.topo, e, w);
        }
    }

    #[inline]
    fn is_single_loop(&self) -> bool {
        self.topo.loop_edge_count() > 0 && self.topo.component_count() == 1
    }
}

fn vertex_count(width: usize, height: usize) -> usize {
    (width + 1) * (height + 1)
}

fn vertex_index((x, y): (usize, usize), width: usize) -> usize {
    y * (width + 1) + x
}

fn add_edge<T: LoopTopology>(topo: &mut T, e: EdgeId, width: usize) {
    let (a, b) = match e {
        EdgeId::H(x, y) => (vertex_index((x, y), width), vertex_index((x + 1, y), width)),
        EdgeId::V(x, y) => (vertex_index((x, y), width), vertex_index((x, y + 1), width)),
    };
    topo.add_loop_edge(a, b);
}
