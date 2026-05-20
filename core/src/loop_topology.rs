/// Incremental connectivity over the loop subgraph's vertices, used by
/// [`SolverLines`](crate::SolverLines) to answer loop queries without rebuilding
/// components from scratch on every propagation pass.
///
/// The interface is deliberately representation-agnostic: the union-find backing
/// can be swapped for an endpoint-linking variant (which exploits the degree-<=2
/// invariant) and benchmarked against this one without touching `SolverLines`.
///
/// Vertices are identified by flat index `y * (width + 1) + x`. The topology only
/// ever sees edge additions; the propagator never clears a loop edge, and
/// backtracking is done by cloning rather than removal.
pub trait LoopTopology: Clone {
    /// Creates an empty topology over `vertex_count` vertices and no edges.
    fn new(vertex_count: usize) -> Self;

    /// Records a loop edge between vertices `a` and `b`.
    fn add_loop_edge(&mut self, a: usize, b: usize);

    /// True iff `a` and `b` are in the same connected component of loop edges.
    fn connected(&self, a: usize, b: usize) -> bool;
}

/// Disjoint-set (union-find) implementation of [`LoopTopology`].
///
/// Union by size without path compression, so `find` (and therefore every query)
/// takes `&self` while the trees stay shallow (~log V). This keeps the loop
/// queries non-mutating, which matters because they run on shared/cloned state
/// during lookahead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsuTopology {
    parent: Vec<u32>,
    size: Vec<u32>,
}

impl DsuTopology {
    fn find(&self, mut x: usize) -> usize {
        while self.parent[x] as usize != x {
            x = self.parent[x] as usize;
        }
        x
    }
}

impl LoopTopology for DsuTopology {
    fn new(vertex_count: usize) -> Self {
        Self {
            parent: (0..vertex_count as u32).collect(),
            size: vec![1; vertex_count],
        }
    }

    fn add_loop_edge(&mut self, a: usize, b: usize) {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        let (big, small) = if self.size[ra] >= self.size[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big as u32;
        self.size[big] += self.size[small];
    }

    fn connected(&self, a: usize, b: usize) -> bool {
        self.find(a) == self.find(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_vertices_are_disconnected() {
        let t = DsuTopology::new(4);
        assert!(!t.connected(0, 1));
        assert!(t.connected(2, 2));
    }

    #[test]
    fn union_connects_transitively() {
        let mut t = DsuTopology::new(4);
        t.add_loop_edge(0, 1);
        t.add_loop_edge(1, 2);
        assert!(t.connected(0, 2));
        assert!(!t.connected(0, 3));
    }

    #[test]
    fn redundant_edge_is_noop() {
        let mut t = DsuTopology::new(3);
        t.add_loop_edge(0, 1);
        t.add_loop_edge(0, 1);
        assert!(t.connected(0, 1));
        assert!(!t.connected(0, 2));
    }
}
