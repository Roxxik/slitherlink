use crate::cell::Cell;
use crate::edge::EdgeState;
use crate::lines::Lines;
use crate::puzzle::Puzzle;

pub fn is_solved(puzzle: &Puzzle, solution: &impl Lines) -> bool {
    assert_eq!(puzzle.width(), solution.width());
    assert_eq!(puzzle.height(), solution.height());
    let w = puzzle.width();
    let h = puzzle.height();

    if !clues_satisfied(puzzle, solution) {
        return false;
    }
    if !vertex_degrees_valid(solution, w, h) {
        return false;
    }
    solution.is_single_loop()
}

fn clues_satisfied(puzzle: &Puzzle, solution: &impl Lines) -> bool {
    for y in 0..puzzle.height() {
        for x in 0..puzzle.width() {
            if let Cell::Clue(n) = puzzle.cell(x, y) {
                if cell_loop_edges(solution, x, y) != n as usize {
                    return false;
                }
            }
        }
    }
    true
}

fn vertex_degrees_valid(solution: &impl Lines, w: usize, h: usize) -> bool {
    for y in 0..=h {
        for x in 0..=w {
            let d = vertex_loop_degree(solution, x, y, w, h);
            if d != 0 && d != 2 {
                return false;
            }
        }
    }
    true
}

fn cell_loop_edges(s: &impl Lines, x: usize, y: usize) -> usize {
    let mut n = 0;
    if s.h_edge(x, y) == EdgeState::Loop {
        n += 1;
    }
    if s.h_edge(x, y + 1) == EdgeState::Loop {
        n += 1;
    }
    if s.v_edge(x, y) == EdgeState::Loop {
        n += 1;
    }
    if s.v_edge(x + 1, y) == EdgeState::Loop {
        n += 1;
    }
    n
}

fn vertex_loop_degree(s: &impl Lines, x: usize, y: usize, w: usize, h: usize) -> usize {
    loop_neighbors(s, x, y, w, h).len()
}

fn loop_neighbors(s: &impl Lines, x: usize, y: usize, w: usize, h: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(2);
    if x > 0 && s.h_edge(x - 1, y) == EdgeState::Loop {
        out.push((x - 1, y));
    }
    if x < w && s.h_edge(x, y) == EdgeState::Loop {
        out.push((x + 1, y));
    }
    if y > 0 && s.v_edge(x, y - 1) == EdgeState::Loop {
        out.push((x, y - 1));
    }
    if y < h && s.v_edge(x, y) == EdgeState::Loop {
        out.push((x, y + 1));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell::{Clue, Empty};
    use crate::lines::PlayLines;

    fn set_loop_h(s: &mut PlayLines, edges: &[(usize, usize)]) {
        for &(x, y) in edges {
            s.set_h_edge(x, y, EdgeState::Loop);
        }
    }

    fn set_loop_v(s: &mut PlayLines, edges: &[(usize, usize)]) {
        for &(x, y) in edges {
            s.set_v_edge(x, y, EdgeState::Loop);
        }
    }

    fn border_2x2_loop() -> PlayLines {
        let mut s = PlayLines::empty(2, 2);
        set_loop_h(&mut s, &[(0, 0), (1, 0), (0, 2), (1, 2)]);
        set_loop_v(&mut s, &[(0, 0), (0, 1), (2, 0), (2, 1)]);
        s
    }

    #[test]
    fn solves_2x2_border_loop() {
        let p = Puzzle::new(2, 2, vec![Clue(2); 4]);
        assert!(is_solved(&p, &border_2x2_loop()));
    }

    #[test]
    fn solves_1x1_loop_around_empty_cell() {
        let p = Puzzle::new(1, 1, vec![Empty]);
        let mut s = PlayLines::empty(1, 1);
        set_loop_h(&mut s, &[(0, 0), (0, 1)]);
        set_loop_v(&mut s, &[(0, 0), (1, 0)]);
        assert!(is_solved(&p, &s));
    }

    #[test]
    fn rejects_empty_solution() {
        let p = Puzzle::new(2, 2, vec![Clue(2); 4]);
        assert!(!is_solved(&p, &PlayLines::empty(2, 2)));
    }

    #[test]
    fn rejects_clue_mismatch() {
        let p = Puzzle::new(2, 2, vec![Clue(3), Clue(2), Clue(2), Clue(2)]);
        assert!(!is_solved(&p, &border_2x2_loop()));
    }

    #[test]
    fn rejects_open_path() {
        let p = Puzzle::new(2, 2, vec![Clue(2); 4]);
        let mut s = PlayLines::empty(2, 2);
        set_loop_h(&mut s, &[(0, 0), (1, 0)]);
        set_loop_v(&mut s, &[(2, 0)]);
        assert!(!is_solved(&p, &s));
    }

    #[test]
    fn rejects_branch_with_matching_clues() {
        let p = Puzzle::new(2, 2, vec![Clue(3); 4]);
        let mut s = border_2x2_loop();
        set_loop_v(&mut s, &[(1, 0), (1, 1)]);
        assert!(!is_solved(&p, &s));
    }

    #[test]
    fn rejects_two_separate_loops() {
        let p: Puzzle = "4x1\na11a".parse().unwrap();
        let mut s = PlayLines::empty(4, 1);
        set_loop_h(&mut s, &[(0, 0), (0, 1), (3, 0), (3, 1)]);
        set_loop_v(&mut s, &[(0, 0), (1, 0), (3, 0), (4, 0)]);
        assert!(!is_solved(&p, &s));
    }

    #[test]
    fn excluded_edges_are_not_loop() {
        let p = Puzzle::new(2, 2, vec![Clue(2); 4]);
        let mut s = border_2x2_loop();
        s.set_v_edge(1, 0, EdgeState::Excluded);
        s.set_v_edge(1, 1, EdgeState::Excluded);
        s.set_h_edge(0, 1, EdgeState::Excluded);
        s.set_h_edge(1, 1, EdgeState::Excluded);
        assert!(is_solved(&p, &s));
    }
}
