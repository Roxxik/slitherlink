use crate::edge::{EdgeId, EdgeState};
use crate::lines::{Lines, PlayLines};

/// Edge-state grid used by the propagator. Wraps [`PlayLines`] and will layer
/// incremental loop bookkeeping on top so loop queries are O(1) instead of
/// rebuilt on demand.
///
/// Unlike [`PlayLines`], it assumes monotonic edits (`Unset` -> `Loop`/`Excluded`
/// only), which the propagator guarantees; it is not meant for arbitrary
/// play-mode mutation or undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolverLines {
    lines: PlayLines,
}

impl SolverLines {
    pub fn empty(width: usize, height: usize) -> Self {
        Self { lines: PlayLines::empty(width, height) }
    }

    /// Drops the bookkeeping and returns the underlying [`PlayLines`].
    pub fn into_play(self) -> PlayLines {
        self.lines
    }
}

impl From<PlayLines> for SolverLines {
    fn from(lines: PlayLines) -> Self {
        Self { lines }
    }
}

impl Lines for SolverLines {
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
        self.lines.set_edge(e, state);
    }
}
