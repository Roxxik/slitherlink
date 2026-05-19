mod common;
mod metropolis;
mod walk;

use crate::cell::Cell;
use crate::check::is_solved;
use crate::propagate::propagate;
use crate::puzzle::Puzzle;
use crate::rng::Rng;

pub use metropolis::{MetropolisBuilder, MetropolisProposal, ProposalAction};
pub use walk::{LotteryRow, WalkBuilder};

/// Which region generator to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionAlgo {
    /// Weighted DFS random walk (see [`WalkBuilder`]).
    Walk,
    /// Pure Metropolis cell-toggle chain (see [`MetropolisBuilder`]).
    Metropolis,
}

/// Generates an easy-difficulty puzzle of the requested size, seeded by `seed`,
/// using the [`Walk`](RegionAlgo::Walk) generator. Convenience wrapper around
/// [`generate_with`].
pub fn generate(width: usize, height: usize, seed: u64) -> Puzzle {
    generate_with(width, height, seed, RegionAlgo::Walk)
}

/// Generates an easy-difficulty puzzle of the requested size, seeded by `seed`,
/// using the chosen region generator.
///
/// "Easy" means the resulting puzzle is solvable end-to-end by `propagate` alone:
/// clues are stripped one by one in random order, and a strip is only kept if
/// propagation still produces a fully-solved board.
pub fn generate_with(width: usize, height: usize, seed: u64, algo: RegionAlgo) -> Puzzle {
    assert!(
        width >= 2 && height >= 2,
        "generate requires at least a 2x2 grid (got {width}x{height})",
    );
    let mut rng = Rng::new(seed);
    let region = match algo {
        RegionAlgo::Walk => walk::run(width, height, &mut rng),
        RegionAlgo::Metropolis => metropolis::run(width, height, &mut rng),
    };
    let full = clues_from_region(width, height, &region);
    strip_clues(full, &mut rng)
}

fn strip_clues(mut puzzle: Puzzle, rng: &mut Rng) -> Puzzle {
    let w = puzzle.width();
    let h = puzzle.height();
    let mut positions: Vec<(usize, usize)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .collect();
    rng.shuffle(&mut positions);
    for (x, y) in positions {
        let saved = puzzle.cell(x, y);
        if matches!(saved, Cell::Empty) {
            continue;
        }
        puzzle.set_cell(x, y, Cell::Empty);
        let sol = propagate(&puzzle);
        if !is_solved(&puzzle, &sol) {
            puzzle.set_cell(x, y, saved);
        }
    }
    puzzle
}

fn clues_from_region(w: usize, h: usize, inside: &[bool]) -> Puzzle {
    let mut cells = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let me = inside[y * w + x];
            let mut n = 0u8;
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                let neighbor_inside = if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    false
                } else {
                    inside[ny as usize * w + nx as usize]
                };
                if me != neighbor_inside {
                    n += 1;
                }
            }
            cells.push(Cell::Clue(n));
        }
    }
    Puzzle::new(w, h, cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge::EdgeState;

    #[test]
    fn clues_for_top_row_region_2x2() {
        // R = top row {(0,0), (1,0)}. The loop wraps the top row.
        // Both top-row cells have 3 of their 4 edges on the loop (top, side, bottom).
        // Both bottom-row cells have 1 (their top, shared with the inside row).
        let inside = vec![true, true, false, false];
        let p = clues_from_region(2, 2, &inside);
        assert_eq!(p.cell(0, 0), Cell::Clue(3));
        assert_eq!(p.cell(1, 0), Cell::Clue(3));
        assert_eq!(p.cell(0, 1), Cell::Clue(1));
        assert_eq!(p.cell(1, 1), Cell::Clue(1));
    }

    #[test]
    fn generated_puzzle_has_clues_in_range() {
        let p = generate(5, 5, 42);
        for c in p.cells() {
            if let Cell::Clue(n) = c {
                assert!(*n <= 3, "clue {n} out of range");
            }
        }
    }

    #[test]
    fn generated_puzzle_is_propagation_solvable() {
        for seed in 1..10 {
            let p = generate(5, 5, seed);
            let sol = propagate(&p);
            assert!(
                is_solved(&p, &sol),
                "seed {seed} did not solve via propagation:\n{}",
                p.overlay(&sol),
            );
        }
    }

    #[test]
    fn generated_puzzle_is_deterministic() {
        let a = generate(6, 6, 12345);
        let b = generate(6, 6, 12345);
        assert_eq!(a, b);
    }

    #[test]
    fn generated_puzzle_strips_at_least_one_clue() {
        // With ample size, at least some clue should be stripped, otherwise the
        // stripping loop is doing nothing useful.
        let p = generate(7, 7, 1);
        let stripped = p.cells().iter().filter(|c| matches!(c, Cell::Empty)).count();
        assert!(stripped > 0, "no clues stripped at all");
    }

    #[test]
    fn solution_loop_consistent_with_solved_puzzle() {
        // Smoke: propagate's loop edges agree with cell-edge counts as clues.
        let p = generate(5, 5, 7);
        let sol = propagate(&p);
        assert!(is_solved(&p, &sol));
        for y in 0..p.height() {
            for x in 0..p.width() {
                if let Cell::Clue(n) = p.cell(x, y) {
                    let mut got = 0u8;
                    if sol.h_edge(x, y) == EdgeState::Loop {
                        got += 1;
                    }
                    if sol.h_edge(x, y + 1) == EdgeState::Loop {
                        got += 1;
                    }
                    if sol.v_edge(x, y) == EdgeState::Loop {
                        got += 1;
                    }
                    if sol.v_edge(x + 1, y) == EdgeState::Loop {
                        got += 1;
                    }
                    assert_eq!(got, n);
                }
            }
        }
    }
}
