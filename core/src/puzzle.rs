use std::fmt;

use crate::cell::Cell;
use crate::edge::EdgeState;
use crate::lines::Lines;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Puzzle {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}

impl Puzzle {
    pub fn new(width: usize, height: usize, cells: Vec<Cell>) -> Self {
        assert_eq!(
            cells.len(),
            width * height,
            "cell count {} does not match {width}x{height}",
            cells.len(),
        );
        Self { width, height, cells }
    }

    pub fn empty(width: usize, height: usize) -> Self {
        Self::new(width, height, vec![Cell::Empty; width * height])
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn cell(&self, x: usize, y: usize) -> Cell {
        self.cells[self.index(x, y)]
    }

    pub fn set_cell(&mut self, x: usize, y: usize, value: Cell) {
        let i = self.index(x, y);
        self.cells[i] = value;
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Renders this puzzle with `solution` overlaid as ASCII. Loop edges are drawn
    /// solid (`---` / `|`), Excluded edges as `x`, Unset edges as blank.
    pub fn overlay(&self, solution: &impl Lines) -> String {
        assert_eq!(self.width, solution.width());
        assert_eq!(self.height, solution.height());
        let mut out = String::new();
        for y in 0..self.height {
            self.append_corner_row(&mut out, solution, y);
            self.append_cell_row(&mut out, solution, y);
        }
        self.append_corner_row(&mut out, solution, self.height);
        out
    }

    fn append_corner_row(&self, out: &mut String, solution: &impl Lines, y: usize) {
        out.push('.');
        for x in 0..self.width {
            let s = match solution.h_edge(x, y) {
                EdgeState::Loop => "---",
                EdgeState::Excluded => " x ",
                EdgeState::Unset => "   ",
            };
            out.push_str(s);
            out.push('.');
        }
        out.push('\n');
    }

    fn append_cell_row(&self, out: &mut String, solution: &impl Lines, y: usize) {
        for x in 0..self.width {
            let v = match solution.v_edge(x, y) {
                EdgeState::Loop => '|',
                EdgeState::Excluded => 'x',
                EdgeState::Unset => ' ',
            };
            out.push(v);
            out.push(' ');
            out.push(match self.cell(x, y) {
                Cell::Empty => ' ',
                Cell::Clue(n) => char::from_digit(n as u32, 10).expect("clue 0..=3"),
            });
            out.push(' ');
        }
        let v = match solution.v_edge(self.width, y) {
            EdgeState::Loop => '|',
            EdgeState::Excluded => 'x',
            EdgeState::Unset => ' ',
        };
        out.push(v);
        out.push('\n');
    }

    pub fn to_storage_string(&self) -> String {
        let mut out = format!("{}x{}\n", self.width, self.height);
        for y in 0..self.height {
            let mut run = 0usize;
            for x in 0..self.width {
                match self.cell(x, y) {
                    Cell::Empty => run += 1,
                    Cell::Clue(n) => {
                        emit_empty_run(&mut out, &mut run);
                        out.push(char::from_digit(n as u32, 10).expect("clue 0..=3"));
                    }
                }
            }
            emit_empty_run(&mut out, &mut run);
            out.push('\n');
        }
        out
    }

    fn index(&self, x: usize, y: usize) -> usize {
        assert!(x < self.width && y < self.height, "cell ({x},{y}) out of bounds {}x{}", self.width, self.height);
        y * self.width + x
    }
}

impl fmt::Display for Puzzle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for y in 0..self.height {
            write_corner_row(f, self.width)?;
            for x in 0..self.width {
                let ch = match self.cell(x, y) {
                    Cell::Empty => ' ',
                    Cell::Clue(n) => char::from_digit(n as u32, 10).expect("clue 0..=3"),
                };
                write!(f, "  {ch} ")?;
            }
            writeln!(f)?;
        }
        write_corner_row(f, self.width)
    }
}

fn write_corner_row(f: &mut fmt::Formatter<'_>, width: usize) -> fmt::Result {
    for _ in 0..width {
        write!(f, ".   ")?;
    }
    writeln!(f, ".")
}

fn emit_empty_run(out: &mut String, run: &mut usize) {
    while *run > 0 {
        let take = (*run).min(26);
        out.push((b'a' + (take - 1) as u8) as char);
        *run -= take;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell::{Clue, Empty};

    fn sample() -> Puzzle {
        Puzzle::new(3, 2, vec![Clue(3), Empty, Clue(2), Empty, Empty, Clue(1)])
    }

    #[test]
    fn pretty_print_format() {
        let mut want = String::new();
        want.push_str(".   .   .   .\n");
        want.push_str("  3       2 \n");
        want.push_str(".   .   .   .\n");
        want.push_str("          1 \n");
        want.push_str(".   .   .   .\n");
        assert_eq!(sample().to_string(), want);
    }

    #[test]
    fn storage_round_trip() {
        let p = sample();
        let serialized = p.to_storage_string();
        assert_eq!(serialized, "3x2\n3a2\nb1\n");
        let reparsed: Puzzle = serialized.parse().unwrap();
        assert_eq!(reparsed, p);
    }

    #[test]
    fn storage_chunks_long_empty_runs() {
        // 30 empty cells in a row -> "z" (26) then "d" (4)
        let mut cells = vec![Empty; 30];
        cells.push(Clue(0));
        let p = Puzzle::new(31, 1, cells);
        assert_eq!(p.to_storage_string(), "31x1\nzd0\n");
    }
}
