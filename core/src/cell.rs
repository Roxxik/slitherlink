#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cell {
    Empty,
    Clue(u8),
}

impl Cell {
    pub fn clue(value: u8) -> Self {
        assert!(value <= 3, "clue value must be in 0..=3, got {value}");
        Self::Clue(value)
    }

    pub fn as_clue(self) -> Option<u8> {
        match self {
            Self::Clue(n) => Some(n),
            Self::Empty => None,
        }
    }
}
