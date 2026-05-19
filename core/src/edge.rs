#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeState {
    #[default]
    Unset,
    Loop,
    Excluded,
}
