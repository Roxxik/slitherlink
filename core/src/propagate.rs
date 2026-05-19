use std::collections::VecDeque;

use crate::cell::Cell;
use crate::check::is_solved;
use crate::edge::EdgeState;
use crate::puzzle::Puzzle;
use crate::solution::Solution;

#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeId {
    H(usize, usize),
    V(usize, usize),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Problems {
    pub bad_cells: Vec<(usize, usize)>,
    pub bad_vertices: Vec<(usize, usize)>,
}

/// Inspects a (possibly partial) solution and reports cells / vertices whose
/// local constraints can no longer be satisfied.
///
/// A cell with clue N is bad if loop edges > N (too many), or 4 - excluded < N
/// (not enough remaining edges to reach N).
///
/// A vertex is bad if it has >= 3 loop edges (impossible degree), or exactly
/// one loop edge with no remaining unset edges (dead end).
pub fn find_problems(puzzle: &Puzzle, sol: &Solution) -> Problems {
    let mut out = Problems::default();
    let w = puzzle.width();
    let h = puzzle.height();

    for y in 0..h {
        for x in 0..w {
            if let Cell::Clue(n) = puzzle.cell(x, y) {
                let n = n as usize;
                let edges = cell_edges(x, y);
                let (loops, excluded, _unset) = split_states(sol, &edges);
                if loops.len() > n || 4 - excluded.len() < n {
                    out.bad_cells.push((x, y));
                }
            }
        }
    }

    for y in 0..=h {
        for x in 0..=w {
            let edges = vertex_edges(x, y, w, h);
            let (loops, _excluded, unset) = split_states(sol, &edges);
            if loops.len() >= 3 || (loops.len() == 1 && unset.is_empty()) {
                out.bad_vertices.push((x, y));
            }
        }
    }

    out
}

/// Runs constraint propagation from scratch.
pub fn propagate(puzzle: &Puzzle) -> Solution {
    let mut sol = Solution::empty(puzzle.width(), puzzle.height());
    propagate_from(puzzle, &mut sol);
    sol
}

/// Applies forcing rules until fixpoint. Never overwrites an already-set edge,
/// so a contradicting deduction is silently dropped (caller can detect by re-checking).
pub fn propagate_from(puzzle: &Puzzle, sol: &mut Solution) {
    apply_pattern_corners(puzzle, sol);
    apply_pattern_adjacent_threes(puzzle, sol);
    apply_pattern_diagonal_threes(puzzle, sol);

    let w = puzzle.width();
    let h = puzzle.height();
    loop {
        let mut changed = false;
        for y in 0..h {
            for x in 0..w {
                if apply_cell_clue(puzzle, sol, x, y, false) {
                    changed = true;
                }
            }
        }
        for y in 0..=h {
            for x in 0..=w {
                if apply_vertex_degree(sol, x, y, w, h, false) {
                    changed = true;
                }
            }
        }
        if apply_no_premature_loop(puzzle, sol) {
            changed = true;
        }
        if !changed {
            return;
        }
    }
}

/// Local auto-exclude pass intended for active play: only sets edges to
/// `Excluded`, never to `Loop`, so the player still draws every line themselves.
///
/// Excludes around clue cells whose loop count is already satisfied, and around
/// vertices that are either capped at degree 2 or stuck at degree 0 with one
/// remaining unset edge.
pub fn auto_exclude(puzzle: &Puzzle, sol: &mut Solution) {
    let w = puzzle.width();
    let h = puzzle.height();
    loop {
        let mut changed = false;
        for y in 0..h {
            for x in 0..w {
                if apply_cell_clue(puzzle, sol, x, y, true) {
                    changed = true;
                }
            }
        }
        for y in 0..=h {
            for x in 0..=w {
                if apply_vertex_degree(sol, x, y, w, h, true) {
                    changed = true;
                }
            }
        }
        if !changed {
            return;
        }
    }
}

fn is_clue(puzzle: &Puzzle, x: usize, y: usize, value: u8) -> bool {
    matches!(puzzle.cell(x, y), Cell::Clue(n) if n == value)
}

/// 1-in-corner forces the corner pair Excluded; 3-in-corner forces them Loop.
fn apply_pattern_corners(puzzle: &Puzzle, sol: &mut Solution) {
    let w = puzzle.width();
    let h = puzzle.height();
    if w == 0 || h == 0 {
        return;
    }
    let corners = [
        (0, 0, EdgeId::H(0, 0), EdgeId::V(0, 0)),
        (w - 1, 0, EdgeId::H(w - 1, 0), EdgeId::V(w, 0)),
        (0, h - 1, EdgeId::H(0, h), EdgeId::V(0, h - 1)),
        (w - 1, h - 1, EdgeId::H(w - 1, h), EdgeId::V(w, h - 1)),
    ];
    for (cx, cy, e1, e2) in corners {
        if is_clue(puzzle, cx, cy, 1) {
            try_set(sol, e1, EdgeState::Excluded);
            try_set(sol, e2, EdgeState::Excluded);
        } else if is_clue(puzzle, cx, cy, 3) {
            try_set(sol, e1, EdgeState::Loop);
            try_set(sol, e2, EdgeState::Loop);
        }
    }
}

/// Two adjacent 3-clues (horizontally or vertically) force 3 edges Loop (the outer
/// edge of each cell plus the shared edge between them) and 2 edges Excluded (the
/// perpendicular extensions of the shared edge on either side, when they exist).
///
/// The shared-edge-is-Loop deduction technically allows a degenerate case where the
/// 2-cell loop *is* the entire puzzle solution, but that never arises in real puzzles.
fn apply_pattern_adjacent_threes(puzzle: &Puzzle, sol: &mut Solution) {
    let w = puzzle.width();
    let h = puzzle.height();
    for y in 0..h {
        for x in 0..w.saturating_sub(1) {
            if is_clue(puzzle, x, y, 3) && is_clue(puzzle, x + 1, y, 3) {
                try_set(sol, EdgeId::V(x, y), EdgeState::Loop);
                try_set(sol, EdgeId::V(x + 1, y), EdgeState::Loop);
                try_set(sol, EdgeId::V(x + 2, y), EdgeState::Loop);
                if y > 0 {
                    try_set(sol, EdgeId::V(x + 1, y - 1), EdgeState::Excluded);
                }
                if y + 1 < h {
                    try_set(sol, EdgeId::V(x + 1, y + 1), EdgeState::Excluded);
                }
            }
        }
    }
    for y in 0..h.saturating_sub(1) {
        for x in 0..w {
            if is_clue(puzzle, x, y, 3) && is_clue(puzzle, x, y + 1, 3) {
                try_set(sol, EdgeId::H(x, y), EdgeState::Loop);
                try_set(sol, EdgeId::H(x, y + 1), EdgeState::Loop);
                try_set(sol, EdgeId::H(x, y + 2), EdgeState::Loop);
                if x > 0 {
                    try_set(sol, EdgeId::H(x - 1, y + 1), EdgeState::Excluded);
                }
                if x + 1 < w {
                    try_set(sol, EdgeId::H(x + 1, y + 1), EdgeState::Excluded);
                }
            }
        }
    }
}

/// Two diagonally adjacent 3-clues force both "outer" edges of each cell
/// (the two edges on the opposite corner from the diagonal) to Loop.
fn apply_pattern_diagonal_threes(puzzle: &Puzzle, sol: &mut Solution) {
    let w = puzzle.width();
    let h = puzzle.height();
    for y in 0..h {
        for x in 0..w {
            if !is_clue(puzzle, x, y, 3) {
                continue;
            }
            // Down-right neighbour (x+1, y+1)
            if x + 1 < w && y + 1 < h && is_clue(puzzle, x + 1, y + 1, 3) {
                try_set(sol, EdgeId::H(x, y), EdgeState::Loop);
                try_set(sol, EdgeId::V(x, y), EdgeState::Loop);
            }
            // Down-left neighbour (x-1, y+1)
            if x > 0 && y + 1 < h && is_clue(puzzle, x - 1, y + 1, 3) {
                try_set(sol, EdgeId::H(x, y), EdgeState::Loop);
                try_set(sol, EdgeId::V(x + 1, y), EdgeState::Loop);
            }
            // Up-right neighbour (x+1, y-1)
            if x + 1 < w && y > 0 && is_clue(puzzle, x + 1, y - 1, 3) {
                try_set(sol, EdgeId::H(x, y + 1), EdgeState::Loop);
                try_set(sol, EdgeId::V(x, y), EdgeState::Loop);
            }
            // Up-left neighbour (x-1, y-1)
            if x > 0 && y > 0 && is_clue(puzzle, x - 1, y - 1, 3) {
                try_set(sol, EdgeId::H(x, y + 1), EdgeState::Loop);
                try_set(sol, EdgeId::V(x + 1, y), EdgeState::Loop);
            }
        }
    }
}

/// Excludes any unset edge whose two endpoints are already connected via Loop edges,
/// unless setting the edge to Loop would complete the puzzle (`is_solved` returns true).
fn apply_no_premature_loop(puzzle: &Puzzle, sol: &mut Solution) -> bool {
    let w = puzzle.width();
    let h = puzzle.height();
    let comp = compute_loop_components(sol, w, h);

    let mut to_exclude: Vec<EdgeId> = Vec::new();
    for y in 0..=h {
        for x in 0..w {
            if sol.h_edge(x, y) != EdgeState::Unset {
                continue;
            }
            if comp[y][x] != comp[y][x + 1] {
                continue;
            }
            let mut tentative = sol.clone();
            tentative.set_h_edge(x, y, EdgeState::Loop);
            if !is_solved(puzzle, &tentative) {
                to_exclude.push(EdgeId::H(x, y));
            }
        }
    }
    for y in 0..h {
        for x in 0..=w {
            if sol.v_edge(x, y) != EdgeState::Unset {
                continue;
            }
            if comp[y][x] != comp[y + 1][x] {
                continue;
            }
            let mut tentative = sol.clone();
            tentative.set_v_edge(x, y, EdgeState::Loop);
            if !is_solved(puzzle, &tentative) {
                to_exclude.push(EdgeId::V(x, y));
            }
        }
    }

    let mut changed = false;
    for e in to_exclude {
        if try_set(sol, e, EdgeState::Excluded) {
            changed = true;
        }
    }
    changed
}

fn compute_loop_components(sol: &Solution, w: usize, h: usize) -> Vec<Vec<usize>> {
    let mut comp = vec![vec![usize::MAX; w + 1]; h + 1];
    let mut next_id = 0;
    for sy in 0..=h {
        for sx in 0..=w {
            if comp[sy][sx] != usize::MAX {
                continue;
            }
            comp[sy][sx] = next_id;
            let mut queue = VecDeque::new();
            queue.push_back((sx, sy));
            while let Some((x, y)) = queue.pop_front() {
                for (nx, ny) in loop_neighbors_at(sol, x, y, w, h) {
                    if comp[ny][nx] == usize::MAX {
                        comp[ny][nx] = next_id;
                        queue.push_back((nx, ny));
                    }
                }
            }
            next_id += 1;
        }
    }
    comp
}

fn loop_neighbors_at(sol: &Solution, x: usize, y: usize, w: usize, h: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(4);
    if x > 0 && sol.h_edge(x - 1, y) == EdgeState::Loop {
        out.push((x - 1, y));
    }
    if x < w && sol.h_edge(x, y) == EdgeState::Loop {
        out.push((x + 1, y));
    }
    if y > 0 && sol.v_edge(x, y - 1) == EdgeState::Loop {
        out.push((x, y - 1));
    }
    if y < h && sol.v_edge(x, y) == EdgeState::Loop {
        out.push((x, y + 1));
    }
    out
}

fn apply_cell_clue(puzzle: &Puzzle, sol: &mut Solution, x: usize, y: usize, excludes_only: bool) -> bool {
    let Cell::Clue(n) = puzzle.cell(x, y) else { return false };
    let n = n as usize;
    let edges = cell_edges(x, y);
    let (loops, excluded, unset) = split_states(sol, &edges);
    if unset.is_empty() {
        return false;
    }
    if loops.len() == n {
        return force_all(sol, &unset, EdgeState::Excluded);
    }
    if !excludes_only && excluded.len() == 4 - n {
        return force_all(sol, &unset, EdgeState::Loop);
    }
    false
}

fn apply_vertex_degree(sol: &mut Solution, x: usize, y: usize, w: usize, h: usize, excludes_only: bool) -> bool {
    let edges = vertex_edges(x, y, w, h);
    let (loops, _excluded, unset) = split_states(sol, &edges);
    if unset.is_empty() {
        return false;
    }
    if loops.len() >= 2 {
        return force_all(sol, &unset, EdgeState::Excluded);
    }
    if !excludes_only && loops.len() == 1 && unset.len() == 1 {
        return force_all(sol, &unset, EdgeState::Loop);
    }
    if loops.is_empty() && unset.len() == 1 {
        return force_all(sol, &unset, EdgeState::Excluded);
    }
    false
}

fn cell_edges(x: usize, y: usize) -> [EdgeId; 4] {
    [
        EdgeId::H(x, y),
        EdgeId::H(x, y + 1),
        EdgeId::V(x, y),
        EdgeId::V(x + 1, y),
    ]
}

fn vertex_edges(x: usize, y: usize, w: usize, h: usize) -> Vec<EdgeId> {
    let mut out = Vec::with_capacity(4);
    if x > 0 {
        out.push(EdgeId::H(x - 1, y));
    }
    if x < w {
        out.push(EdgeId::H(x, y));
    }
    if y > 0 {
        out.push(EdgeId::V(x, y - 1));
    }
    if y < h {
        out.push(EdgeId::V(x, y));
    }
    out
}

fn get_edge(sol: &Solution, e: EdgeId) -> EdgeState {
    match e {
        EdgeId::H(x, y) => sol.h_edge(x, y),
        EdgeId::V(x, y) => sol.v_edge(x, y),
    }
}

fn try_set(sol: &mut Solution, e: EdgeId, state: EdgeState) -> bool {
    if get_edge(sol, e) != EdgeState::Unset {
        return false;
    }
    match e {
        EdgeId::H(x, y) => sol.set_h_edge(x, y, state),
        EdgeId::V(x, y) => sol.set_v_edge(x, y, state),
    }
    true
}

fn force_all(sol: &mut Solution, edges: &[EdgeId], state: EdgeState) -> bool {
    let mut changed = false;
    for &e in edges {
        if try_set(sol, e, state) {
            changed = true;
        }
    }
    changed
}

fn split_states(sol: &Solution, edges: &[EdgeId]) -> (Vec<EdgeId>, Vec<EdgeId>, Vec<EdgeId>) {
    let mut loops = Vec::new();
    let mut excluded = Vec::new();
    let mut unset = Vec::new();
    for &e in edges {
        match get_edge(sol, e) {
            EdgeState::Loop => loops.push(e),
            EdgeState::Excluded => excluded.push(e),
            EdgeState::Unset => unset.push(e),
        }
    }
    (loops, excluded, unset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell::{Clue, Empty};

    fn count_loop(sol: &Solution) -> usize {
        let mut n = 0;
        for y in 0..=sol.height() {
            for x in 0..sol.width() {
                if sol.h_edge(x, y) == EdgeState::Loop {
                    n += 1;
                }
            }
        }
        for y in 0..sol.height() {
            for x in 0..=sol.width() {
                if sol.v_edge(x, y) == EdgeState::Loop {
                    n += 1;
                }
            }
        }
        n
    }

    fn count_excluded(sol: &Solution) -> usize {
        let mut n = 0;
        for y in 0..=sol.height() {
            for x in 0..sol.width() {
                if sol.h_edge(x, y) == EdgeState::Excluded {
                    n += 1;
                }
            }
        }
        for y in 0..sol.height() {
            for x in 0..=sol.width() {
                if sol.v_edge(x, y) == EdgeState::Excluded {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn empty_puzzle_no_deductions() {
        let p = Puzzle::new(3, 3, vec![Empty; 9]);
        let s = propagate(&p);
        assert_eq!(count_loop(&s), 0);
        assert_eq!(count_excluded(&s), 0);
    }

    #[test]
    fn zero_clue_excludes_all_four_edges() {
        let mut cells = vec![Empty; 9];
        cells[4] = Clue(0); // centre cell
        let p = Puzzle::new(3, 3, cells);
        let s = propagate(&p);
        assert_eq!(s.h_edge(1, 1), EdgeState::Excluded);
        assert_eq!(s.h_edge(1, 2), EdgeState::Excluded);
        assert_eq!(s.v_edge(1, 1), EdgeState::Excluded);
        assert_eq!(s.v_edge(2, 1), EdgeState::Excluded);
        assert_eq!(count_loop(&s), 0);
    }

    #[test]
    fn adjacent_zero_and_three_propagates() {
        // 1x2 grid: cell(0,0) = 0, cell(1,0) = 3
        // After 0-rule: cell(0,0)'s 4 edges excluded, including v(1,0) which is also cell(1,0)'s left edge.
        // After cell-clue rule on cell(1,0): excluded=1, n=3, 4-n=1 -> force remaining 3 unset edges to Loop.
        let p = Puzzle::new(2, 1, vec![Clue(0), Clue(3)]);
        let s = propagate(&p);
        assert_eq!(s.v_edge(1, 0), EdgeState::Excluded);
        assert_eq!(s.h_edge(1, 0), EdgeState::Loop);
        assert_eq!(s.h_edge(1, 1), EdgeState::Loop);
        assert_eq!(s.v_edge(2, 0), EdgeState::Loop);
    }

    #[test]
    fn vertex_corner_forces_dead_end() {
        // 1x1 grid with 0 clue: corner vertex (0,0) has edges h(0,0) and v(0,0), both Excluded after rule.
        // Vertex degree rule fires on those vertices: 0 loop + 0 unset -> nothing to do.
        // Sanity check there are no spurious Loop deductions.
        let p = Puzzle::new(1, 1, vec![Clue(0)]);
        let s = propagate(&p);
        assert_eq!(count_loop(&s), 0);
        assert_eq!(count_excluded(&s), 4);
    }

    #[test]
    fn detects_3_next_to_0_as_unsolvable() {
        // 1x2: clue 3, clue 0. With the corner pattern, the 3 forces its corner pair Loop;
        // the 0 forces all four of its edges Excluded. That leaves dead-end vertices.
        let p = Puzzle::new(2, 1, vec![Clue(3), Clue(0)]);
        let s = propagate(&p);
        let problems = find_problems(&p, &s);
        let any_problem = !problems.bad_cells.is_empty() || !problems.bad_vertices.is_empty();
        assert!(any_problem, "expected some problem to be flagged");
    }

    #[test]
    fn detects_manual_dead_end_vertex() {
        // 2x2 empty puzzle. Manually create an L=1, U=0 configuration at vertex (1, 0)
        // by marking one incident edge Loop and the rest Excluded.
        let p = Puzzle::new(2, 2, vec![Empty; 4]);
        let mut s = Solution::empty(2, 2);
        s.set_h_edge(0, 0, EdgeState::Loop);
        s.set_h_edge(1, 0, EdgeState::Excluded);
        s.set_v_edge(1, 0, EdgeState::Excluded);
        let problems = find_problems(&p, &s);
        assert!(problems.bad_vertices.contains(&(1, 0)));
    }

    #[test]
    fn no_problems_on_open_grid() {
        let p = Puzzle::new(3, 3, vec![Empty; 9]);
        let s = propagate(&p);
        let problems = find_problems(&p, &s);
        assert!(problems.bad_cells.is_empty());
        assert!(problems.bad_vertices.is_empty());
    }

    #[test]
    fn detects_clue_overflow() {
        // 2x2 with a clue-3 cell where we manually exclude two of its edges.
        // Now max possible loop count = 4 - 2 = 2 < 3 -> bad cell.
        let p = Puzzle::new(2, 2, vec![Clue(3), Empty, Empty, Empty]);
        let mut s = Solution::empty(2, 2);
        s.set_h_edge(0, 0, EdgeState::Excluded);
        s.set_v_edge(0, 0, EdgeState::Excluded);
        let problems = find_problems(&p, &s);
        assert!(problems.bad_cells.contains(&(0, 0)));
    }

    #[test]
    fn three_in_corner_forces_pair_loop() {
        // Empty puzzle with a single 3 in the top-left corner.
        let mut cells = vec![Empty; 9];
        cells[0] = Clue(3);
        let p = Puzzle::new(3, 3, cells);
        let s = propagate(&p);
        assert_eq!(s.h_edge(0, 0), EdgeState::Loop);
        assert_eq!(s.v_edge(0, 0), EdgeState::Loop);
    }

    #[test]
    fn one_in_corner_forces_pair_excluded() {
        let mut cells = vec![Empty; 9];
        cells[0] = Clue(1);
        let p = Puzzle::new(3, 3, cells);
        let s = propagate(&p);
        assert_eq!(s.h_edge(0, 0), EdgeState::Excluded);
        assert_eq!(s.v_edge(0, 0), EdgeState::Excluded);
    }

    #[test]
    fn adjacent_horizontal_threes_force_three_loops() {
        // 4x1 with 3,3 in the middle two cells: outer vertical edges AND the shared
        // middle edge forced Loop. Top/bottom row, so no perpendicular extensions.
        let p = Puzzle::new(4, 1, vec![Empty, Clue(3), Clue(3), Empty]);
        let s = propagate(&p);
        assert_eq!(s.v_edge(1, 0), EdgeState::Loop);
        assert_eq!(s.v_edge(2, 0), EdgeState::Loop);
        assert_eq!(s.v_edge(3, 0), EdgeState::Loop);
    }

    #[test]
    fn adjacent_horizontal_threes_force_extensions_excluded() {
        // 4x3 with 3,3 in middle row; perpendicular extensions of the shared edge
        // (above and below) forced Excluded.
        let mut cells = vec![Empty; 12];
        cells[5] = Clue(3); // (1, 1)
        cells[6] = Clue(3); // (2, 1)
        let p = Puzzle::new(4, 3, cells);
        let s = propagate(&p);
        assert_eq!(s.v_edge(2, 0), EdgeState::Excluded);
        assert_eq!(s.v_edge(2, 2), EdgeState::Excluded);
    }

    #[test]
    fn adjacent_vertical_threes_force_three_loops() {
        let p = Puzzle::new(1, 4, vec![Empty, Clue(3), Clue(3), Empty]);
        let s = propagate(&p);
        assert_eq!(s.h_edge(0, 1), EdgeState::Loop);
        assert_eq!(s.h_edge(0, 2), EdgeState::Loop);
        assert_eq!(s.h_edge(0, 3), EdgeState::Loop);
    }

    #[test]
    fn diagonal_threes_force_outer_pair_loops() {
        // 3x3 with 3 at (0,0) and 3 at (1,1) - down-right diagonal.
        let mut cells = vec![Empty; 9];
        cells[0] = Clue(3); // (0,0)
        cells[4] = Clue(3); // (1,1)
        let p = Puzzle::new(3, 3, cells);
        let s = propagate(&p);
        // (0,0) outer: top + left = h(0,0), v(0,0)
        assert_eq!(s.h_edge(0, 0), EdgeState::Loop);
        assert_eq!(s.v_edge(0, 0), EdgeState::Loop);
        // (1,1) outer: bottom + right = h(1,2), v(2,1)
        assert_eq!(s.h_edge(1, 2), EdgeState::Loop);
        assert_eq!(s.v_edge(2, 1), EdgeState::Loop);
    }

    #[test]
    fn no_premature_loop_excludes_closing_edge() {
        // 3x1 with cell (2,0) clue 1. The other two cells are empty.
        // Pre-mark three sides of cell (0,0)'s border as Loop, leaving v(1,0) Unset.
        // Closing v(1,0) would form a tiny loop around (0,0) but never satisfy
        // the clue at (2,0), so v(1,0) must be Excluded.
        let p = Puzzle::new(3, 1, vec![Empty, Empty, Clue(1)]);
        let mut s = Solution::empty(3, 1);
        s.set_h_edge(0, 0, EdgeState::Loop);
        s.set_h_edge(0, 1, EdgeState::Loop);
        s.set_v_edge(0, 0, EdgeState::Loop);
        propagate_from(&p, &mut s);
        assert_eq!(s.v_edge(1, 0), EdgeState::Excluded);
    }

    #[test]
    fn closing_edge_kept_when_it_completes_puzzle() {
        // 1x1 empty puzzle: three border edges of the only cell already Loop.
        // The fourth completes the unique solution; the rule must not exclude it.
        let p = Puzzle::new(1, 1, vec![Empty]);
        let mut s = Solution::empty(1, 1);
        s.set_h_edge(0, 0, EdgeState::Loop);
        s.set_h_edge(0, 1, EdgeState::Loop);
        s.set_v_edge(0, 0, EdgeState::Loop);
        propagate_from(&p, &mut s);
        // Vertex rules will actually force v(1, 0) to Loop here, but in any case
        // the no-premature-loop rule must not Exclude it.
        assert_ne!(s.v_edge(1, 0), EdgeState::Excluded);
    }

    #[test]
    fn propagate_from_partial_state() {
        // 2x2 with no clues, but manually mark 2 adjacent loop edges at vertex (1, 0).
        // Vertex (1, 0) now has loop>=2 -> remaining unset edges at (1, 0) get Excluded.
        let p = Puzzle::new(2, 2, vec![Empty; 4]);
        let mut s = Solution::empty(2, 2);
        s.set_h_edge(0, 0, EdgeState::Loop);
        s.set_h_edge(1, 0, EdgeState::Loop);
        propagate_from(&p, &mut s);
        assert_eq!(s.v_edge(1, 0), EdgeState::Excluded);
    }

    #[test]
    fn auto_exclude_caps_satisfied_clue() {
        // 2x2 with a 1-clue at (0, 0). Player marks h(0, 0) as Loop, satisfying
        // the clue. auto_exclude must X the remaining three edges around (0, 0)
        // but must not draw any new Loop edges anywhere.
        let p = Puzzle::new(2, 2, vec![Clue(1), Empty, Empty, Empty]);
        let mut s = Solution::empty(2, 2);
        s.set_h_edge(0, 0, EdgeState::Loop);
        let loops_before = count_loop(&s);
        auto_exclude(&p, &mut s);
        assert_eq!(s.h_edge(0, 1), EdgeState::Excluded);
        assert_eq!(s.v_edge(0, 0), EdgeState::Excluded);
        assert_eq!(s.v_edge(1, 0), EdgeState::Excluded);
        assert_eq!(count_loop(&s), loops_before, "auto_exclude must not add Loop edges");
    }

    #[test]
    fn auto_exclude_does_not_complete_clue() {
        // 2x2 with a 3-clue at (0, 0). Two of its edges are already Excluded;
        // propagate would force the remaining two to Loop. auto_exclude must not.
        let p = Puzzle::new(2, 2, vec![Clue(3), Empty, Empty, Empty]);
        let mut s = Solution::empty(2, 2);
        s.set_h_edge(0, 1, EdgeState::Excluded);
        s.set_v_edge(1, 0, EdgeState::Excluded);
        auto_exclude(&p, &mut s);
        assert_eq!(s.h_edge(0, 0), EdgeState::Unset);
        assert_eq!(s.v_edge(0, 0), EdgeState::Unset);
    }

    #[test]
    fn auto_exclude_x_dead_end_vertex() {
        // 2x2 with no clues. Mark three of vertex (1, 0)'s edges as Excluded.
        // The remaining h(0, 0) cannot be Loop (would leave degree 1), so X it.
        let p = Puzzle::new(2, 2, vec![Empty; 4]);
        let mut s = Solution::empty(2, 2);
        s.set_h_edge(1, 0, EdgeState::Excluded);
        s.set_v_edge(1, 0, EdgeState::Excluded);
        auto_exclude(&p, &mut s);
        assert_eq!(s.h_edge(0, 0), EdgeState::Excluded);
    }

    #[test]
    fn auto_exclude_does_not_extend_at_vertex() {
        // Vertex (1, 0) on a 2x2 empty puzzle has one Loop edge; propagate would
        // force the only remaining unset edge to Loop to satisfy degree 2.
        // auto_exclude must leave it Unset.
        let p = Puzzle::new(2, 2, vec![Empty; 4]);
        let mut s = Solution::empty(2, 2);
        s.set_h_edge(0, 0, EdgeState::Loop);
        s.set_h_edge(1, 0, EdgeState::Excluded);
        auto_exclude(&p, &mut s);
        assert_eq!(s.v_edge(1, 0), EdgeState::Unset);
    }
}
