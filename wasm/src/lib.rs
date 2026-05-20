use slitherlink_core as core;
use slitherlink_core::Lines as _;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum EdgeState {
    Unset = 0,
    Loop = 1,
    Excluded = 2,
}

impl From<EdgeState> for core::EdgeState {
    fn from(s: EdgeState) -> Self {
        match s {
            EdgeState::Unset => Self::Unset,
            EdgeState::Loop => Self::Loop,
            EdgeState::Excluded => Self::Excluded,
        }
    }
}

impl From<core::EdgeState> for EdgeState {
    fn from(s: core::EdgeState) -> Self {
        match s {
            core::EdgeState::Unset => Self::Unset,
            core::EdgeState::Loop => Self::Loop,
            core::EdgeState::Excluded => Self::Excluded,
        }
    }
}

#[wasm_bindgen]
pub struct Puzzle {
    inner: core::Puzzle,
}

#[wasm_bindgen]
impl Puzzle {
    pub fn parse(s: &str) -> Result<Puzzle, JsError> {
        s.parse::<core::Puzzle>()
            .map(|inner| Self { inner })
            .map_err(|e| JsError::new(&e.to_string()))
    }

    pub fn empty(width: usize, height: usize) -> Puzzle {
        Self { inner: core::Puzzle::empty(width, height) }
    }

    pub fn width(&self) -> usize {
        self.inner.width()
    }

    pub fn height(&self) -> usize {
        self.inner.height()
    }

    /// Returns the clue at (x, y), or `undefined` for empty cells.
    pub fn clue(&self, x: usize, y: usize) -> Option<u8> {
        match self.inner.cell(x, y) {
            core::Cell::Empty => None,
            core::Cell::Clue(n) => Some(n),
        }
    }

    pub fn pretty(&self) -> String {
        format!("{}", self.inner)
    }

    pub fn storage(&self) -> String {
        self.inner.to_storage_string()
    }

    #[wasm_bindgen(js_name = "setClue")]
    pub fn set_clue(&mut self, x: usize, y: usize, clue: Option<u8>) -> Result<(), JsError> {
        let cell = match clue {
            None => core::Cell::Empty,
            Some(n) if n <= 3 => core::Cell::Clue(n),
            Some(n) => return Err(JsError::new(&format!("clue {n} out of range"))),
        };
        self.inner.set_cell(x, y, cell);
        Ok(())
    }
}

#[wasm_bindgen]
pub struct Solution {
    inner: core::PlayLines,
}

#[wasm_bindgen]
impl Solution {
    pub fn empty(width: usize, height: usize) -> Solution {
        Self { inner: core::PlayLines::empty(width, height) }
    }

    pub fn width(&self) -> usize {
        self.inner.width()
    }

    pub fn height(&self) -> usize {
        self.inner.height()
    }

    #[wasm_bindgen(js_name = "hEdge")]
    pub fn h_edge(&self, x: usize, y: usize) -> EdgeState {
        self.inner.h_edge(x, y).into()
    }

    #[wasm_bindgen(js_name = "vEdge")]
    pub fn v_edge(&self, x: usize, y: usize) -> EdgeState {
        self.inner.v_edge(x, y).into()
    }

    #[wasm_bindgen(js_name = "setHEdge")]
    pub fn set_h_edge(&mut self, x: usize, y: usize, state: EdgeState) {
        self.inner.set_h_edge(x, y, state.into());
    }

    #[wasm_bindgen(js_name = "setVEdge")]
    pub fn set_v_edge(&mut self, x: usize, y: usize, state: EdgeState) {
        self.inner.set_v_edge(x, y, state.into());
    }
}

#[wasm_bindgen(js_name = "isSolved")]
pub fn is_solved(puzzle: &Puzzle, solution: &Solution) -> bool {
    core::is_solved(&puzzle.inner, &solution.inner)
}

#[wasm_bindgen]
pub fn propagate(puzzle: &Puzzle) -> Solution {
    Solution { inner: core::propagate(&puzzle.inner).into_play() }
}

#[wasm_bindgen(js_name = "autoExclude")]
pub fn auto_exclude(puzzle: &Puzzle, solution: &mut Solution) {
    core::auto_exclude(&puzzle.inner, &mut solution.inner);
}

#[wasm_bindgen]
pub struct Problems {
    inner: core::Problems,
}

#[wasm_bindgen]
impl Problems {
    #[wasm_bindgen(js_name = "badCells")]
    pub fn bad_cells(&self) -> Vec<u32> {
        flatten_pairs(&self.inner.bad_cells)
    }

    #[wasm_bindgen(js_name = "badVertices")]
    pub fn bad_vertices(&self) -> Vec<u32> {
        flatten_pairs(&self.inner.bad_vertices)
    }
}

fn flatten_pairs(pairs: &[(usize, usize)]) -> Vec<u32> {
    let mut out = Vec::with_capacity(pairs.len() * 2);
    for &(x, y) in pairs {
        out.push(x as u32);
        out.push(y as u32);
    }
    out
}

#[wasm_bindgen(js_name = "findProblems")]
pub fn find_problems(puzzle: &Puzzle, solution: &Solution) -> Problems {
    Problems { inner: core::find_problems(&puzzle.inner, &solution.inner) }
}
