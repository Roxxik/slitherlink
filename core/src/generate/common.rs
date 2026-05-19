use std::collections::VecDeque;

use crate::rng::Rng;

pub(super) fn pick_target_size(total: usize, rng: &mut Rng) -> usize {
    let lo = ((total * 55) / 100).max(2);
    let hi = ((total * 70) / 100).max(lo + 1);
    lo + rng.range(hi - lo + 1)
}

pub(super) fn neighbors(x: usize, y: usize, w: usize, h: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::with_capacity(4);
    if x > 0 {
        out.push((x - 1, y));
    }
    if x + 1 < w {
        out.push((x + 1, y));
    }
    if y > 0 {
        out.push((x, y - 1));
    }
    if y + 1 < h {
        out.push((x, y + 1));
    }
    out
}

/// Checks that `inside` represents a valid simply-connected region for
/// Slitherlink loop generation: at least 2 cells in R, at least 1 outside R,
/// R is connected, and the complement is connected through the grid exterior
/// (i.e. R has no holes).
pub(super) fn is_valid_region(w: usize, h: usize, inside: &[bool]) -> bool {
    let count = inside.iter().filter(|b| **b).count();
    if count < 2 || count == w * h {
        return false;
    }
    region_connected(w, h, inside, true, count)
        && complement_connected(w, h, inside, w * h - count)
}

pub(super) fn region_connected(
    w: usize,
    h: usize,
    inside: &[bool],
    value: bool,
    total: usize,
) -> bool {
    let start = inside.iter().position(|&b| b == value);
    let Some(start) = start else {
        return total == 0;
    };
    let mut visited = vec![false; w * h];
    visited[start] = true;
    let mut queue = VecDeque::new();
    queue.push_back(start);
    let mut count = 1;
    while let Some(i) = queue.pop_front() {
        let x = i % w;
        let y = i / w;
        for (nx, ny) in neighbors(x, y, w, h) {
            let ni = ny * w + nx;
            if inside[ni] == value && !visited[ni] {
                visited[ni] = true;
                count += 1;
                queue.push_back(ni);
            }
        }
    }
    count == total
}

/// True if R has no holes — equivalently, every outside cell can reach the
/// grid exterior without crossing R. We model the grid exterior as one virtual
/// node connected to *every* border outside cell, so two outside cells in
/// otherwise-disconnected in-grid clusters still count as connected as long
/// as they both touch the grid border.
fn complement_connected(w: usize, h: usize, inside: &[bool], total_outside: usize) -> bool {
    if total_outside == 0 {
        return false;
    }
    let mut visited = vec![false; w * h];
    let mut queue = VecDeque::new();
    let mut count = 0usize;
    for y in 0..h {
        for x in 0..w {
            let on_border = x == 0 || y == 0 || x == w - 1 || y == h - 1;
            if on_border && !inside[y * w + x] {
                let i = y * w + x;
                if !visited[i] {
                    visited[i] = true;
                    queue.push_back(i);
                    count += 1;
                }
            }
        }
    }
    if queue.is_empty() {
        // All border cells are in R, but some interior cells are outside ->
        // those are enclosed -> hole.
        return false;
    }
    while let Some(i) = queue.pop_front() {
        let x = i % w;
        let y = i / w;
        for (nx, ny) in neighbors(x, y, w, h) {
            let ni = ny * w + nx;
            if !inside[ni] && !visited[ni] {
                visited[ni] = true;
                count += 1;
                queue.push_back(ni);
            }
        }
    }
    count == total_outside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_validity_rejects_singletons() {
        let mut inside = vec![false; 9];
        inside[4] = true;
        assert!(!is_valid_region(3, 3, &inside));
    }

    #[test]
    fn region_validity_rejects_hole() {
        // 3x3 ring: outer 8 cells inside, centre outside -> complement has a hole.
        let mut inside = vec![true; 9];
        inside[4] = false;
        assert!(!is_valid_region(3, 3, &inside));
    }

    #[test]
    fn region_validity_rejects_disconnected() {
        // Two diagonal cells, no connecting cell.
        let mut inside = vec![false; 9];
        inside[0] = true;
        inside[8] = true;
        assert!(!is_valid_region(3, 3, &inside));
    }

    #[test]
    fn region_validity_accepts_simple_pair() {
        let mut inside = vec![false; 9];
        inside[4] = true;
        inside[5] = true;
        assert!(is_valid_region(3, 3, &inside));
    }
}
