use super::common::{is_valid_region, pick_target_size, region_connected};
use crate::rng::Rng;

pub(super) fn run(w: usize, h: usize, rng: &mut Rng) -> Vec<bool> {
    let seed = rng.next_u64();
    let mut b = MetropolisBuilder::new(w, h, seed);
    while b.step() {}
    b.into_inside()
}

/// One Metropolis proposal — what cell was picked, which way we tried to
/// toggle it, and whether the toggle was accepted.
#[derive(Debug, Clone)]
pub struct MetropolisProposal {
    pub cell: (usize, usize),
    pub action: ProposalAction,
    pub accepted: bool,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalAction {
    Add,
    Remove,
}

/// Pure Metropolis-style region generator: each step picks a uniformly random
/// cell, proposes to toggle its inside/outside state, and accepts iff the
/// resulting region is still valid (≥2 cells, ≤ w·h-1, R connected, complement
/// hole-free).
///
/// No size bias — the chain hovers near 50 % of the grid by symmetry, which
/// lines up with the walk-based generator's target band. `done()` uses the
/// same condition as the walk (target met AND all four borders touched), so
/// final states between the two generators can be compared.
pub struct MetropolisBuilder {
    width: usize,
    height: usize,
    inside: Vec<bool>,
    target: usize,
    rng: Rng,
    iteration: usize,
    max_iterations: usize,
    last_proposal: Option<MetropolisProposal>,
}

impl MetropolisBuilder {
    pub fn new(width: usize, height: usize, seed: u64) -> Self {
        assert!(width >= 2 && height >= 2, "region grid must be at least 2x2");
        let mut rng = Rng::new(seed);
        let total = width * height;
        let target = pick_target_size(total, &mut rng);
        let start = rng.range(total);
        let x0 = start % width;
        let y0 = start / width;
        let mut dirs = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)].to_vec();
        rng.shuffle(&mut dirs);
        let mut neighbour = None;
        for (dx, dy) in dirs {
            let nx = x0 as i32 + dx;
            let ny = y0 as i32 + dy;
            if nx >= 0 && ny >= 0 && nx < width as i32 && ny < height as i32 {
                neighbour = Some((nx as usize, ny as usize));
                break;
            }
        }
        let mut inside = vec![false; total];
        inside[y0 * width + x0] = true;
        if let Some((nx, ny)) = neighbour {
            inside[ny * width + nx] = true;
        }
        let max_iterations = total * 100;
        Self {
            width,
            height,
            inside,
            target,
            rng,
            iteration: 0,
            max_iterations,
            last_proposal: None,
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

    pub fn target(&self) -> usize {
        self.target
    }

    pub fn count(&self) -> usize {
        self.inside.iter().filter(|b| **b).count()
    }

    pub fn iteration(&self) -> usize {
        self.iteration
    }

    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }

    pub fn last_proposal(&self) -> Option<&MetropolisProposal> {
        self.last_proposal.as_ref()
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

    pub fn step(&mut self) -> bool {
        if self.done() {
            return false;
        }
        if self.iteration >= self.max_iterations {
            return false;
        }
        let total = self.width * self.height;
        let i = self.rng.range(total);
        let x = i % self.width;
        let y = i / self.width;
        let was_inside = self.inside[i];
        let action = if was_inside {
            ProposalAction::Remove
        } else {
            ProposalAction::Add
        };
        self.inside[i] = !was_inside;
        let (accepted, note) = if is_valid_region(self.width, self.height, &self.inside) {
            (true, String::from("valid"))
        } else {
            self.inside[i] = was_inside;
            let count = self.inside.iter().filter(|b| **b).count();
            let new_count = if was_inside { count - 1 } else { count + 1 };
            let reason = if new_count < 2 {
                "size < 2"
            } else if new_count == total {
                "size == grid"
            } else if !region_connected(self.width, self.height, &{
                let mut probe = self.inside.clone();
                probe[i] = !was_inside;
                probe
            }, true, new_count) {
                "R disconnected"
            } else {
                "complement has hole"
            };
            (false, reason.to_string())
        };
        self.last_proposal = Some(MetropolisProposal {
            cell: (x, y),
            action,
            accepted,
            note,
        });
        self.iteration += 1;
        true
    }

    pub(super) fn into_inside(self) -> Vec<bool> {
        self.inside
    }
}
