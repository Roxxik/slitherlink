use std::fmt;
use std::str::FromStr;

use crate::cell::Cell;
use crate::puzzle::Puzzle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    MissingHeaderNewline,
    InvalidHeader(String),
    UnexpectedChar { line: usize, col: usize, ch: char },
    TooManyCells { line: usize, col: usize, expected: usize },
    TooFewCells { expected: usize, got: usize },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeaderNewline => write!(f, "header line must be terminated by a newline"),
            Self::InvalidHeader(s) => write!(f, "invalid header {s:?}, expected WIDTHxHEIGHT"),
            Self::UnexpectedChar { line, col, ch } => {
                write!(f, "unexpected character {ch:?} at line {line}, column {col}")
            }
            Self::TooManyCells { line, col, expected } => {
                write!(f, "too many cells (expected {expected}) at line {line}, column {col}")
            }
            Self::TooFewCells { expected, got } => {
                write!(f, "too few cells: got {got}, expected {expected}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl FromStr for Puzzle {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (header, body) = s.split_once('\n').ok_or(ParseError::MissingHeaderNewline)?;
        let (width, height) = parse_header(header)?;
        let expected = width * height;
        let mut cells: Vec<Cell> = Vec::with_capacity(expected);
        let mut line = 2usize;
        let mut col = 0usize;
        for ch in body.chars() {
            col += 1;
            match ch {
                '\n' => {
                    line += 1;
                    col = 0;
                }
                '\r' => {}
                '0'..='3' => {
                    if cells.len() == expected {
                        return Err(ParseError::TooManyCells { line, col, expected });
                    }
                    cells.push(Cell::Clue(ch.to_digit(10).unwrap() as u8));
                }
                'a'..='z' => {
                    let n = (ch as u8 - b'a' + 1) as usize;
                    if cells.len() + n > expected {
                        return Err(ParseError::TooManyCells { line, col, expected });
                    }
                    for _ in 0..n {
                        cells.push(Cell::Empty);
                    }
                }
                _ => return Err(ParseError::UnexpectedChar { line, col, ch }),
            }
        }
        if cells.len() != expected {
            return Err(ParseError::TooFewCells { expected, got: cells.len() });
        }
        Ok(Puzzle::new(width, height, cells))
    }
}

fn parse_header(s: &str) -> Result<(usize, usize), ParseError> {
    let s = s.trim_end_matches('\r');
    let (w, h) = s.split_once('x').ok_or_else(|| ParseError::InvalidHeader(s.to_string()))?;
    let width: usize = w.parse().map_err(|_| ParseError::InvalidHeader(s.to_string()))?;
    let height: usize = h.parse().map_err(|_| ParseError::InvalidHeader(s.to_string()))?;
    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell::{Clue, Empty};

    #[test]
    fn parses_with_row_newlines() {
        let p: Puzzle = "3x2\n3a2\nb1\n".parse().unwrap();
        assert_eq!(p.width(), 3);
        assert_eq!(p.height(), 2);
        assert_eq!(p.cells(), &[Clue(3), Empty, Clue(2), Empty, Empty, Clue(1)]);
    }

    #[test]
    fn parses_without_row_newlines() {
        let p: Puzzle = "3x2\n3a2b1".parse().unwrap();
        assert_eq!(p.cells(), &[Clue(3), Empty, Clue(2), Empty, Empty, Clue(1)]);
    }

    #[test]
    fn parses_with_extra_newlines() {
        let p: Puzzle = "3x2\n\n3a2\n\nb1\n\n".parse().unwrap();
        assert_eq!(p.cells(), &[Clue(3), Empty, Clue(2), Empty, Empty, Clue(1)]);
    }

    #[test]
    fn parses_run_crossing_row_boundary() {
        let p: Puzzle = "3x2\nf".parse().unwrap();
        assert_eq!(p.cells(), &[Empty; 6]);
    }

    #[test]
    fn parses_crlf() {
        let p: Puzzle = "3x2\r\n3a2\r\nb1\r\n".parse().unwrap();
        assert_eq!(p.cells(), &[Clue(3), Empty, Clue(2), Empty, Empty, Clue(1)]);
    }

    #[test]
    fn rejects_missing_header_newline() {
        assert_eq!("3x2".parse::<Puzzle>(), Err(ParseError::MissingHeaderNewline));
    }

    #[test]
    fn rejects_bad_header() {
        let err = "3y2\n".parse::<Puzzle>().unwrap_err();
        assert!(matches!(err, ParseError::InvalidHeader(_)));
    }

    #[test]
    fn rejects_unexpected_char() {
        let err = "3x2\n3a2\nb!1".parse::<Puzzle>().unwrap_err();
        assert_eq!(err, ParseError::UnexpectedChar { line: 3, col: 2, ch: '!' });
    }

    #[test]
    fn rejects_too_many_cells() {
        let err = "3x2\n3a2\nb12".parse::<Puzzle>().unwrap_err();
        assert!(matches!(err, ParseError::TooManyCells { expected: 6, .. }));
    }

    #[test]
    fn rejects_too_few_cells() {
        let err = "3x2\n3a2\nb".parse::<Puzzle>().unwrap_err();
        assert_eq!(err, ParseError::TooFewCells { expected: 6, got: 5 });
    }

    #[test]
    fn rejects_clue_out_of_range() {
        let err = "3x2\n3a2\nb4".parse::<Puzzle>().unwrap_err();
        assert_eq!(err, ParseError::UnexpectedChar { line: 3, col: 2, ch: '4' });
    }
}
