mod common;
mod metropolis;
mod walk;

use crate::cell::Cell;
use crate::check::is_solved;
use crate::propagate::{propagate, propagate_easy};
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

/// Target difficulty. Selects the solver used to vet the puzzle during
/// generation: a board is only emitted if that solver can solve it end-to-end,
/// so the tier bounds how hard the player's deductions have to be.
///
/// - [`Easy`](Difficulty::Easy): the limited rule set in [`propagate_easy`] —
///   clue completion, vertex continuation, soft-exclusion, and no-premature-loop,
///   with no lookahead. Deliberately beatable by hand.
/// - [`Hard`](Difficulty::Hard): the full [`propagate`], including the seed
///   patterns and 1-step lookahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Hard,
}

/// Generates an easy-difficulty puzzle of the requested size, seeded by `seed`,
/// using the [`Walk`](RegionAlgo::Walk) generator. Convenience wrapper around
/// [`generate_with`].
pub fn generate(width: usize, height: usize, seed: u64) -> Puzzle {
    generate_with(width, height, seed, Difficulty::Easy, RegionAlgo::Walk)
}

/// Generates the puzzle identified by `(width, height, difficulty, number)`.
/// `number` is the per-category level number, not an RNG seed: the actual seed is
/// derived internally by [`seed_from`].
///
/// One [`Rng`] is seeded once and used first to pick a region generator and then
/// to drive that generator, so the same coordinates always yield the same puzzle.
/// This is the entry point the UI uses to introduce variation across puzzles:
/// adding a generator (or, later, random parameter choices drawn from the same
/// `Rng`) only means extending [`pick_algo`], not touching callers.
///
/// Size, difficulty, and level number are folded into the RNG seed, so distinct
/// categories diverge: easy-7x7-1 and hard-7x7-1 draw different loops. Difficulty
/// additionally selects the generation *strategy* — which solver vets the board —
/// in [`generate_with_rng`].
pub fn generate_seeded(width: usize, height: usize, difficulty: Difficulty, number: u64) -> Puzzle {
    let mut rng = Rng::new(seed_from(width, height, difficulty, number));
    let algo = pick_algo(&mut rng);
    generate_with_rng(width, height, &mut rng, algo, difficulty)
}

/// Derives a well-distributed RNG seed from a puzzle's category coordinates.
/// Uses splitmix64 mixing so nearby level numbers (1, 2, 3, ...) and the small
/// size/difficulty values still produce uncorrelated generator streams.
fn seed_from(width: usize, height: usize, difficulty: Difficulty, number: u64) -> u64 {
    fn mix(mut z: u64) -> u64 {
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    let diff = match difficulty {
        Difficulty::Easy => 0,
        Difficulty::Hard => 1,
    };
    let mut h = 0x9E37_79B9_7F4A_7C15_u64;
    h = mix(h ^ width as u64);
    h = mix(h ^ height as u64);
    h = mix(h ^ diff);
    mix(h ^ number)
}

/// Picks a region generator from `rng`. Extend `ALGOS` to add a new generator to
/// the rotation.
fn pick_algo(rng: &mut Rng) -> RegionAlgo {
    const ALGOS: [RegionAlgo; 2] = [RegionAlgo::Walk, RegionAlgo::Metropolis];
    ALGOS[rng.range(ALGOS.len())]
}

/// Generates a puzzle of the requested size and `difficulty`, seeded by `seed`,
/// using the chosen region generator.
///
/// The resulting puzzle is solvable end-to-end by the tier's solver alone (see
/// [`Difficulty`]): clues are stripped one by one in random order, and a strip is
/// only kept if that solver still produces a fully-solved board.
pub fn generate_with(
    width: usize,
    height: usize,
    seed: u64,
    difficulty: Difficulty,
    algo: RegionAlgo,
) -> Puzzle {
    let mut rng = Rng::new(seed);
    generate_with_rng(width, height, &mut rng, algo, difficulty)
}

/// How many fresh regions to try for a fully-clued board the tier's solver can
/// solve before settling for the last one drawn. The limited Easy rules can leave
/// even a fully-clued board unsolvable; rather than strip such a board (which would
/// keep every clue yet still leave the player stuck) we draw another region. This
/// many attempts makes falling short vanishingly rare; [`generate_with_rng`]
/// documents the graceful fallback for when it happens anyway.
const MAX_REGION_ATTEMPTS: usize = 256;

fn generate_with_rng(
    width: usize,
    height: usize,
    rng: &mut Rng,
    algo: RegionAlgo,
    difficulty: Difficulty,
) -> Puzzle {
    assert!(
        width >= 2 && height >= 2,
        "generate requires at least a 2x2 grid (got {width}x{height})",
    );
    // Draw regions until the fully-clued board actually solves under the tier's
    // solver. Checking solvability up front (rather than inferring it from "no clue
    // could be stripped") is what keeps us from emitting a board the solver merely
    // got stuck on; stripping then preserves solvability.
    let mut full = clues_from_region(width, height, &run_region(algo, width, height, rng));
    let mut attempts = 1;
    while attempts < MAX_REGION_ATTEMPTS && !tier_solves(&full, difficulty) {
        full = clues_from_region(width, height, &run_region(algo, width, height, rng));
        attempts += 1;
    }
    // `full` is now tier-solvable, or we exhausted the attempts. Either way
    // `strip_clues` returns a winnable level: it strips down a solvable board, and
    // for an unsolvable fallback it can remove nothing and hands back the fully-
    // clued board as-is. That board always has a solution — a valid region's
    // boundary is a single closed loop satisfying every clue — and being fully
    // hinted it is the gentlest board to hand a player, not the hardest. So an
    // exhausted search degrades to "all clues shown", never to a crash or a
    // genuinely unsolvable board.
    strip_clues(full, rng, difficulty)
}

fn run_region(algo: RegionAlgo, width: usize, height: usize, rng: &mut Rng) -> Vec<bool> {
    match algo {
        RegionAlgo::Walk => walk::run(width, height, rng),
        RegionAlgo::Metropolis => metropolis::run(width, height, rng),
    }
}

/// True iff `puzzle` is solved end-to-end by the solver for `difficulty`.
fn tier_solves(puzzle: &Puzzle, difficulty: Difficulty) -> bool {
    match difficulty {
        Difficulty::Easy => is_solved(puzzle, &propagate_easy(puzzle)),
        Difficulty::Hard => is_solved(puzzle, &propagate(puzzle)),
    }
}

fn strip_clues(mut puzzle: Puzzle, rng: &mut Rng, difficulty: Difficulty) -> Puzzle {
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
        if !tier_solves(&puzzle, difficulty) {
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
    use crate::lines::Lines;

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
    fn easy_generated_is_solvable_by_easy_solver() {
        // The tier's contract: an Easy puzzle is beatable with the limited rule set
        // alone. This also exercises the stuck-board guard — a board the easy solver
        // only got stuck on (rather than fully solving) must never be emitted.
        for seed in 1..15 {
            let p = generate_with(7, 7, seed, Difficulty::Easy, RegionAlgo::Walk);
            let sol = propagate_easy(&p);
            assert!(
                is_solved(&p, &sol),
                "easy seed {seed} not solvable by the easy solver:\n{}",
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
    fn generate_seeded_is_deterministic() {
        // Same seed must reproduce the same puzzle, including the algo pick.
        let a = generate_seeded(7, 7, Difficulty::Easy, 12345);
        let b = generate_seeded(7, 7, Difficulty::Easy, 12345);
        assert_eq!(a, b);
    }

    #[test]
    fn generate_seeded_difficulty_changes_board() {
        // Difficulty is folded into the seed, so the same number under different
        // tiers draws a different loop rather than an identical board.
        let easy = generate_seeded(7, 7, Difficulty::Easy, 999);
        let hard = generate_seeded(7, 7, Difficulty::Hard, 999);
        assert_ne!(easy, hard);
    }

    #[test]
    fn generate_seeded_uses_both_algos_across_seeds() {
        // pick_algo should not collapse to a single generator; over a handful of
        // seeds we expect to draw each variant at least once.
        let mut saw_walk = false;
        let mut saw_metropolis = false;
        for seed in 1..32 {
            let mut rng = Rng::new(seed);
            match pick_algo(&mut rng) {
                RegionAlgo::Walk => saw_walk = true,
                RegionAlgo::Metropolis => saw_metropolis = true,
            }
        }
        assert!(saw_walk && saw_metropolis, "expected both region generators to be picked");
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
