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
    /// Edges set since they were last drained, used as the propagator's worklist:
    /// a freshly-set edge is the only thing that can unlock a new local deduction,
    /// so re-checking is confined to the sites incident to these. Because edits are
    /// monotonic (`Unset` -> set, once per edge), each edge is enqueued at most once,
    /// which keeps the queue bounded and free of duplicates.
    dirty: Vec<EdgeId>,
}

impl<T: LoopTopology> SolverLines<T> {
    pub fn empty(width: usize, height: usize) -> Self {
        Self {
            lines: PlayLines::empty(width, height),
            topo: T::new(vertex_count(width, height)),
            dirty: Vec::new(),
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

    /// Pops the next edge whose incident sites still need re-checking, or `None`
    /// once the worklist is drained.
    pub fn pop_dirty(&mut self) -> Option<EdgeId> {
        self.dirty.pop()
    }
}

impl<T: LoopTopology> From<PlayLines> for SolverLines<T> {
    fn from(lines: PlayLines) -> Self {
        let w = lines.width();
        let h = lines.height();
        let mut topo = T::new(vertex_count(w, h));
        let mut dirty = Vec::new();
        for y in 0..=h {
            for x in 0..w {
                seed_existing(&lines, &mut topo, &mut dirty, EdgeId::H(x, y), w);
            }
        }
        for y in 0..h {
            for x in 0..=w {
                seed_existing(&lines, &mut topo, &mut dirty, EdgeId::V(x, y), w);
            }
        }
        Self { lines, topo, dirty }
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
        if prev == EdgeState::Unset && state != EdgeState::Unset {
            self.dirty.push(e);
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

/// Replays one pre-existing edge into a freshly built [`SolverLines`]: a `Loop`
/// edge joins the topology, and any non-`Unset` edge is enqueued so the propagator
/// re-derives the deductions its state allows (mirrors what `set_edge` records for
/// live edits).
fn seed_existing<T: LoopTopology>(lines: &PlayLines, topo: &mut T, dirty: &mut Vec<EdgeId>, e: EdgeId, width: usize) {
    match lines.edge(e) {
        EdgeState::Loop => {
            add_edge(topo, e, width);
            dirty.push(e);
        }
        EdgeState::Excluded => dirty.push(e),
        EdgeState::Unset => {}
    }
}

fn add_edge<T: LoopTopology>(topo: &mut T, e: EdgeId, width: usize) {
    let (a, b) = match e {
        EdgeId::H(x, y) => (vertex_index((x, y), width), vertex_index((x + 1, y), width)),
        EdgeId::V(x, y) => (vertex_index((x, y), width), vertex_index((x, y + 1), width)),
    };
    topo.add_loop_edge(a, b);
}
