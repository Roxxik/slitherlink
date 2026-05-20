use super::common::{is_valid_region, neighbors, pick_target_size};
use crate::rng::Rng;

pub(super) fn run(w: usize, h: usize, rng: &mut Rng) -> Vec<bool> {
    let seed = rng.next_u64();
    let mut b = WalkBuilder::new(w, h, seed);
    while b.step() {}
    b.into_inside()
}

/// One row of the per-step weighted lottery: a stop option plus one entry per
/// candidate direction. Returned by [`WalkBuilder::next_lottery`] so callers
/// can inspect exactly what the next step will sample from.
#[derive(Debug, Clone)]
pub struct LotteryRow {
    pub label: &'static str,
    pub weight: u32,
    pub note: String,
    /// `Some(cell)` for a move; `None` for the stop row.
    pub target: Option<(usize, usize)>,
}

/// Builds a random simply-connected "inside" region step by step.
///
/// Algorithm: start at a random cell, then do a weighted random walk that
/// never revisits an R cell. At each step we look at the four neighbours of
/// the current tip; each candidate neighbour gets a weight that depends on
/// how many of *its* other neighbours are already in R:
///
/// | other R neighbours | weight | meaning                                |
/// |-------------------:|-------:|----------------------------------------|
/// | 0                  | 16     | exploring fresh ground                 |
/// | 1                  |  4     | grazing an existing R cell             |
/// | 2                  |  1     | wedging into an inlet                  |
/// | 3                  |  0     | filling a deep pocket (rejected)       |
///
/// Adds that would disconnect the complement (create a hole in the outside
/// region) are also rejected. A constant `stop` option (weight 2) is always
/// in the lottery — when filling is the only legal move, the walk usually
/// stops here and restarts from another random boundary R cell. Loop exits
/// when the target size has been reached **and** R touches all four grid
/// borders.
pub struct WalkBuilder {
    width: usize,
    height: usize,
    inside: Vec<bool>,
    target: usize,
    tip: Option<(usize, usize)>,
    rng: Rng,
    stuck: usize,
}

impl WalkBuilder {
    pub fn new(width: usize, height: usize, seed: u64) -> Self {
        assert!(width >= 2 && height >= 2, "region grid must be at least 2x2");
        let mut rng = Rng::new(seed);
        let total = width * height;
        let target = pick_target_size(total, &mut rng);
        let start = rng.range(total);
        let mut inside = vec![false; total];
        inside[start] = true;
        Self {
            width,
            height,
            inside,
            target,
            tip: Some((start % width, start / width)),
            rng,
            stuck: 0,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn inside(&self) -> &[bool] {
        &self.inside
    }

    pub fn tip(&self) -> Option<(usize, usize)> {
        self.tip
    }

    pub fn target(&self) -> usize {
        self.target
    }

    pub fn count(&self) -> usize {
        self.inside.iter().filter(|b| **b).count()
    }

    pub fn all_borders_touched(&self) -> bool {
        let (w, h) = (self.width, self.height);
        (0..w).any(|x| self.inside[x])
            && (0..w).any(|x| self.inside[(h - 1) * w + x])
            && (0..h).any(|y| self.inside[y * w])
            && (0..h).any(|y| self.inside[y * w + w - 1])
    }

    pub fn done(&self) -> bool {
        self.count() >= self.target && self.all_borders_touched()
    }

    /// Performs one step. Returns false when there's no more progress to make
    /// (either `done()`, R is the whole grid, or we've restarted too often
    /// without successfully extending).
    pub fn step(&mut self) -> bool {
        if self.done() {
            return false;
        }
        if self.count() >= self.width * self.height {
            return false;
        }
        let tip = match self.tip {
            Some(t) => t,
            None => match self.pick_boundary_r_cell() {
                Some(c) => {
                    self.tip = Some(c);
                    return true;
                }
                None => return false,
            },
        };
        match self.try_weighted_step(tip) {
            Some(next) => {
                self.tip = Some(next);
                self.stuck = 0;
                true
            }
            None => {
                self.stuck += 1;
                if self.stuck > self.width * self.height {
                    return false;
                }
                self.tip = self.pick_boundary_r_cell();
                true
            }
        }
    }

    /// Returns the lottery rows the next call to [`WalkBuilder::step`] would
    /// sample from. The state is unchanged. Returns `None` if there is no
    /// current tip (the next step will pick a fresh tip first).
    pub fn next_lottery(&mut self) -> Option<Vec<LotteryRow>> {
        let tip = self.tip?;
        Some(self.compute_lottery(tip))
    }

    fn compute_lottery(&mut self, tip: (usize, usize)) -> Vec<LotteryRow> {
        const LABELS: [(&str, (i32, i32)); 4] =
            [("E", (1, 0)), ("W", (-1, 0)), ("S", (0, 1)), ("N", (0, -1))];
        let mut rows = vec![LotteryRow {
            label: "stop",
            weight: Self::STOP_WEIGHT,
            note: String::from("-"),
            target: None,
        }];
        for (label, (dx, dy)) in LABELS {
            rows.push(self.evaluate_direction(tip, label, dx, dy));
        }
        rows
    }

    fn evaluate_direction(
        &mut self,
        tip: (usize, usize),
        label: &'static str,
        dx: i32,
        dy: i32,
    ) -> LotteryRow {
        let (w, h) = (self.width, self.height);
        let nx = tip.0 as i32 + dx;
        let ny = tip.1 as i32 + dy;
        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
            return LotteryRow { label, weight: 0, note: String::from("off-grid"), target: None };
        }
        let nx = nx as usize;
        let ny = ny as usize;
        let idx = ny * w + nx;
        if self.inside[idx] {
            return LotteryRow { label, weight: 0, note: String::from("in R"), target: None };
        }
        let other_inside = neighbors(nx, ny, w, h)
            .into_iter()
            .filter(|&(ax, ay)| (ax, ay) != tip && self.inside[ay * w + ax])
            .count();
        let (weight, kind) = match other_inside {
            0 => (16u32, "fresh"),
            1 => (4, "graze 1"),
            2 => (1, "wedge 2"),
            _ => (0, "pocket 3"),
        };
        if weight == 0 {
            return LotteryRow {
                label,
                weight: 0,
                note: kind.to_string(),
                target: None,
            };
        }
        self.inside[idx] = true;
        let valid = is_valid_region(w, h, &self.inside);
        self.inside[idx] = false;
        if !valid {
            return LotteryRow {
                label,
                weight: 0,
                note: format!("{kind}, splits outside"),
                target: None,
            };
        }
        LotteryRow {
            label,
            weight,
            note: kind.to_string(),
            target: Some((nx, ny)),
        }
    }

    const STOP_WEIGHT: u32 = 2;

    fn try_weighted_step(&mut self, tip: (usize, usize)) -> Option<(usize, usize)> {
        let rows = self.compute_lottery(tip);
        let total: u32 = rows.iter().map(|r| r.weight).sum();
        if total == 0 {
            return None;
        }
        let mut pick = self.rng.range(total as usize) as u32;
        for row in &rows {
            if pick < row.weight {
                return match row.target {
                    Some(cell) => {
                        self.inside[cell.1 * self.width + cell.0] = true;
                        Some(cell)
                    }
                    None => None, // stop picked
                };
            }
            pick -= row.weight;
        }
        unreachable!()
    }

    /// Picks a random R cell that still has at least one outside neighbour,
    /// weighted by the *square* of that cell's outside-neighbour count. So a
    /// cell sticking out with 3 outside neighbours is 9x more likely to be
    /// chosen than a buried cell with only 1 — restarts prefer promising
    /// "leaf" R cells over half-surrounded ones.
    fn pick_boundary_r_cell(&mut self) -> Option<(usize, usize)> {
        let (w, h) = (self.width, self.height);
        let mut weighted: Vec<(u32, (usize, usize))> = Vec::new();
        for i in 0..w * h {
            if !self.inside[i] {
                continue;
            }
            let x = i % w;
            let y = i / w;
            let outside_count = neighbors(x, y, w, h)
                .into_iter()
                .filter(|&(nx, ny)| !self.inside[ny * w + nx])
                .count() as u32;
            if outside_count == 0 {
                continue;
            }
            weighted.push((outside_count * outside_count, (x, y)));
        }
        if weighted.is_empty() {
            return None;
        }
        let total: u32 = weighted.iter().map(|(w_, _)| w_).sum();
        let mut pick = self.rng.range(total as usize) as u32;
        for &(w_, cell) in &weighted {
            if pick < w_ {
                return Some(cell);
            }
            pick -= w_;
        }
        unreachable!()
    }

    pub(super) fn into_inside(self) -> Vec<bool> {
        self.inside
    }
}
