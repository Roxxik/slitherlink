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

    /// Total number of loop edges added.
    fn loop_edge_count(&self) -> usize;

    /// Number of connected components that contain at least one loop edge.
    fn component_count(&self) -> usize;
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
    edges: usize,
    /// Components with at least one loop edge. A vertex carries an edge iff its
    /// component has size >= 2, since the only way to grow a component is to add
    /// an edge incident to both merged sides.
    components: usize,
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
            edges: 0,
            components: 0,
        }
    }

    fn add_loop_edge(&mut self, a: usize, b: usize) {
        self.edges += 1;
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return;
        }
        let a_active = self.size[ra] >= 2;
        let b_active = self.size[rb] >= 2;
        let (big, small) = if self.size[ra] >= self.size[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big as u32;
        self.size[big] += self.size[small];
        match (a_active, b_active) {
            (false, false) => self.components += 1, // two fresh vertices start a component
            (true, true) => self.components -= 1,   // two existing components merge
            _ => {}                                 // edge extends an existing component
        }
    }

    fn connected(&self, a: usize, b: usize) -> bool {
        self.find(a) == self.find(b)
    }

    fn loop_edge_count(&self) -> usize {
        self.edges
    }

    fn component_count(&self) -> usize {
        self.components
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

    #[test]
    fn counts_edges_and_components() {
        let mut t = DsuTopology::new(6);
        assert_eq!(t.loop_edge_count(), 0);
        assert_eq!(t.component_count(), 0);

        // Chain 0-1-2 is one component with two edges.
        t.add_loop_edge(0, 1);
        t.add_loop_edge(1, 2);
        assert_eq!(t.loop_edge_count(), 2);
        assert_eq!(t.component_count(), 1);

        // A separate edge 3-4 is a second component.
        t.add_loop_edge(3, 4);
        assert_eq!(t.loop_edge_count(), 3);
        assert_eq!(t.component_count(), 2);

        // Bridging the two chains merges them back to one component.
        t.add_loop_edge(2, 3);
        assert_eq!(t.loop_edge_count(), 4);
        assert_eq!(t.component_count(), 1);
    }

    #[test]
    fn closing_a_cycle_keeps_one_component() {
        let mut t = DsuTopology::new(4);
        t.add_loop_edge(0, 1);
        t.add_loop_edge(1, 2);
        t.add_loop_edge(2, 3);
        t.add_loop_edge(3, 0); // closes the loop
        assert_eq!(t.loop_edge_count(), 4);
        assert_eq!(t.component_count(), 1);
    }
}
