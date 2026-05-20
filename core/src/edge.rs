#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeState {
    #[default]
    Unset,
    Loop,
    Excluded,
}

/// Identifies an edge in a [`Solution`](crate::Solution): horizontal between
/// vertices (x, y) and (x+1, y), or vertical between (x, y) and (x, y+1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeId {
    H(usize, usize),
    V(usize, usize),
}
