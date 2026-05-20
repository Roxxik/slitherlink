// --- Categories (edit freely; add Large/Huge sizes or tiers later) ---
// Each puzzle is identified by (difficulty, size, number). The id strings are
// what get persisted, so renaming one orphans existing saves — fine in dev.
const DIFFICULTIES = [
  { id: 'easy', label: 'Easy' },
  { id: 'hard', label: 'Hard' },
];
const SIZES = [
  { id: 'tiny', label: 'Tiny', w: 5, h: 5 },
  { id: 'small', label: 'Small', w: 7, h: 7 },
  { id: 'medium', label: 'Medium', w: 10, h: 10 },
];
const DEFAULT_DIFF = 'easy';
const DEFAULT_SIZE = 'small';

// --- Persistence (localStorage) ---
// Two keys hold everything: in-progress boards and solved-puzzle stats. No
// version field by design — on a parse/shape mismatch we surface a wipe banner.
const STORE_ACTIVE = 'slitherlink:active';
const STORE_STATS = 'slitherlink:stats';
const gameId = (difficulty, w, h, number) => `${difficulty}:${w}x${h}:${number}`;

const SVG_NS = 'http://www.w3.org/2000/svg';

// --- Layout ---
const CELL_PX = 80;        // upper bound on on-screen cell size; viewBox expands beyond puzzle bounds on big screens
const PADDING = 0.5;       // grid-space padding around the puzzle inside the SVG viewBox

// --- Zoom (custom, SVG-only) ---
const MIN_ZOOM = 0.6;
const MAX_ZOOM = 8;
const WHEEL_SENSITIVITY = 0.001;
const PAN_THRESHOLD = 8;         // pixels of movement before a 1-finger touch / mouse-down becomes a pan
const PAN_VISIBLE_MARGIN = 1.5;    // min units of puzzle that must stay inside the viewBox

// --- Hit targets ---
const EDGE_HIT_WIDTH = 0.3;  // tap target thickness for edges (perpendicular to the edge)
const EDGE_HIT_CLIP = 0.15;  // shorten edge tap targets at each end so corners go to cells
const CELL_HIT_PAD = 0.15;   // inset of the cell tap target from cell borders
const HIT_DEBUG = false;     // outline hit targets in red

// --- Edge appearance (all in grid-space units) ---
const LOOP_STROKE = '#222';
const BAD_LOOP_STROKE = '#e60000';  // vertex deg >=3, clue overflow, or premature closed loop
const LOOP_WIDTH = 0.08;
const EXCLUDED_STROKE = '#888';
const EXCLUDED_WIDTH = 0.02;
const EXCLUDED_ARM = 0.08;       // half-length of each X arm
const UNSET_STROKE = '#888';     // dotted "no edge committed yet"
const SOFT_STROKE = '#ececec';   // dotted, paler — auto-derived exclusion
const HINT_STROKE_WIDTH = 0.02;  // line width for both Unset and Soft dots
const HINT_DASH = '0.08 0.08';   // dash pattern for both Unset and Soft

// --- Backgrounds ---
const BG_INSIDE = '#ffffff';     // puzzle area (puzzle bounds + PADDING half-cell border)
const BG_OUTSIDE = '#f0eae0';    // everything outside that area inside the SVG canvas

// --- Vertex dots ---
const VERTEX_FILL = '#222';
const VERTEX_RADIUS = 0.06;

// --- Clue numbers ---
const CLUE_FILL_FINISHED = '#222';    // clue's loop-edge count matches its value
const CLUE_FILL_UNFINISHED = '#a13030';// dark red while the clue isn't yet satisfied
const CLUE_FONT_SIZE = 0.5;

// --- Cell highlight colors (cycled by tapping a cell) ---
const CELL_COLOR_CYCLE = [null, 'green', 'blue', 'pink', 'yellow'];
const CELL_COLOR_FILL = {
  green: '#c8eac8',
  blue: '#cce0f5',
  pink: '#f5d6e5',
  yellow: '#f5edc8',
};

// 4-state UI model. Core only knows Unset/Loop/Excluded; `soft` is rendered
// when an edge is Excluded in the Solution but absent from `userEdges`.
const DISPLAY_UNSET = 'unset';
const DISPLAY_LOOP = 'loop';
const DISPLAY_EXCLUDED = 'excluded';
const DISPLAY_SOFT = 'soft';

let puzzle;
let solution;
let cellColors;
// edgeKey -> 'L' | 'X'  (user's canonical input; SoftExclude never appears)
let userEdges;
// Snapshots of {userEdges, cellColors} for undo/redo. Each click pushes onto
// undoStack and clears redoStack; undo moves a snapshot from undo to redo.
let undoStack = [];
let redoStack = [];
// Camera state — viewBox `{x, y, w, h}` persisted across renders. `null` = use base from puzzle dims.
let viewBoxState = null;
let baseViewBox = null;

// The category + number of the puzzle currently shown. `solved` locks the board.
let currentDiff = DEFAULT_DIFF;
let gridW = 7;
let gridH = 7;
let puzzleNumber = 1;
// The current puzzle's clue layout (RLE storage string). Cached in the active
// record so resuming parses it instead of rerunning the slow generator.
let currentPuzzleStorage = null;
let solved = false;
// Timer. `elapsedMs` is solving time banked from finished run segments; while a
// segment is active `timerStartedAt` holds its Date.now() start. The live total
// is `elapsedMs + (now - timerStartedAt)`. The interval refreshes the display
// and flushes progress so a reload resumes near the right time.
let elapsedMs = 0;
let timerStartedAt = null;
let timerInterval = null;

// --- View + home-screen state ---
let view = 'home';            // 'home' | 'board'
let selectedDiff = DEFAULT_DIFF;
let selectedSizeId = DEFAULT_SIZE;

// --- Persisted stores (mirrored in memory; written through on change) ---
let activeGames = {};         // gameId -> in-progress record
let solves = [];              // array of { difficulty, w, h, number, timeMs, solvedAt }
let storageCorrupt = false;   // set during load when a key fails to parse/validate

function svgEl(name, attrs = {}) {
  const el = document.createElementNS(SVG_NS, name);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  return el;
}

function key(x, y) {
  return `${x},${y}`;
}

function edgeKey(axis, x, y) {
  return `${axis}:${x},${y}`;
}

function cellLoopCount(x, y) {
  const { EdgeState } = window.wasmBindings;
  let n = 0;
  if (solution.hEdge(x, y) === EdgeState.Loop) n++;
  if (solution.hEdge(x, y + 1) === EdgeState.Loop) n++;
  if (solution.vEdge(x, y) === EdgeState.Loop) n++;
  if (solution.vEdge(x + 1, y) === EdgeState.Loop) n++;
  return n;
}

// Loop edges incident to vertex (vx, vy), as `[axis, x, y]` triples.
function vertexLoopEdges(vx, vy) {
  const { EdgeState } = window.wasmBindings;
  const w = puzzle.width();
  const h = puzzle.height();
  const out = [];
  if (vx > 0 && solution.hEdge(vx - 1, vy) === EdgeState.Loop) out.push(['h', vx - 1, vy]);
  if (vx < w && solution.hEdge(vx, vy) === EdgeState.Loop) out.push(['h', vx, vy]);
  if (vy > 0 && solution.vEdge(vx, vy - 1) === EdgeState.Loop) out.push(['v', vx, vy - 1]);
  if (vy < h && solution.vEdge(vx, vy) === EdgeState.Loop) out.push(['v', vx, vy]);
  return out;
}

function cellLoopEdges(x, y) {
  const { EdgeState } = window.wasmBindings;
  const edges = [['h', x, y], ['h', x, y + 1], ['v', x, y], ['v', x + 1, y]];
  return edges.filter(([a, ex, ey]) =>
    (a === 'h' ? solution.hEdge(ex, ey) : solution.vEdge(ex, ey)) === EdgeState.Loop);
}

// Returns the set of edgeKeys that should render in red:
//   - any loop edge incident to a vertex with degree >= 3
//   - all four loop edges around a clue cell whose loop count already exceeds the clue
//   - every edge of a closed loop component, when the puzzle isn't yet solved
function findBadEdges() {
  const { isSolved } = window.wasmBindings;
  const w = puzzle.width();
  const h = puzzle.height();
  const bad = new Set();

  for (let y = 0; y <= h; y++) {
    for (let x = 0; x <= w; x++) {
      const edges = vertexLoopEdges(x, y);
      if (edges.length >= 3) {
        for (const [a, ex, ey] of edges) bad.add(edgeKey(a, ex, ey));
      }
    }
  }

  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const clue = puzzle.clue(x, y);
      if (clue === undefined) continue;
      const edges = cellLoopEdges(x, y);
      if (edges.length > clue) {
        for (const [a, ex, ey] of edges) bad.add(edgeKey(a, ex, ey));
      }
    }
  }

  // Closed-loop check: walk each component of the loop-edge graph; if every
  // vertex has degree exactly 2 it's a cycle, which is only legal when the
  // whole puzzle is solved.
  if (!isSolved(puzzle, solution)) {
    const visited = new Set();
    for (let sy = 0; sy <= h; sy++) {
      for (let sx = 0; sx <= w; sx++) {
        const startKey = key(sx, sy);
        if (visited.has(startKey)) continue;
        visited.add(startKey);
        if (vertexLoopEdges(sx, sy).length === 0) continue;
        const compEdges = new Set();
        const queue = [[sx, sy]];
        let allDeg2 = true;
        while (queue.length > 0) {
          const [vx, vy] = queue.shift();
          const incident = vertexLoopEdges(vx, vy);
          if (incident.length !== 2) allDeg2 = false;
          for (const [a, ex, ey] of incident) {
            compEdges.add(edgeKey(a, ex, ey));
            const [nx, ny] = a === 'h'
              ? [ex === vx ? ex + 1 : ex, ey]
              : [ex, ey === vy ? ey + 1 : ey];
            const nKey = key(nx, ny);
            if (!visited.has(nKey)) {
              visited.add(nKey);
              queue.push([nx, ny]);
            }
          }
        }
        if (allDeg2) for (const e of compEdges) bad.add(e);
      }
    }
  }

  return bad;
}

function edgeDisplayState(axis, x, y) {
  const { EdgeState } = window.wasmBindings;
  const state = axis === 'h' ? solution.hEdge(x, y) : solution.vEdge(x, y);
  if (state === EdgeState.Loop) return DISPLAY_LOOP;
  if (state === EdgeState.Excluded) {
    return userEdges.has(edgeKey(axis, x, y)) ? DISPLAY_EXCLUDED : DISPLAY_SOFT;
  }
  return DISPLAY_UNSET;
}

function cycleDisplay(state) {
  // unset / soft -> loop -> excluded -> unset
  if (state === DISPLAY_LOOP) return DISPLAY_EXCLUDED;
  if (state === DISPLAY_EXCLUDED) return DISPLAY_UNSET;
  return DISPLAY_LOOP;
}

function rebuildSolution() {
  const { Solution, EdgeState, autoExclude } = window.wasmBindings;
  if (solution) solution.free();
  solution = Solution.empty(puzzle.width(), puzzle.height());
  for (const [k, mark] of userEdges) {
    const [axis, coord] = k.split(':');
    const [x, y] = coord.split(',').map(Number);
    const edge = mark === 'L' ? EdgeState.Loop : EdgeState.Excluded;
    if (axis === 'h') solution.setHEdge(x, y, edge);
    else solution.setVEdge(x, y, edge);
  }
  autoExclude(puzzle, solution);
}

function svgPixelSize() {
  const container = document.getElementById('puzzle');
  return {
    width: Math.max(120, container.clientWidth),
    height: Math.max(120, container.clientHeight),
  };
}

// Picks a viewBox that matches the canvas aspect ratio (no letterboxing),
// shows the whole puzzle, and caps the cell size at CELL_PX on big screens.
function computeInitialViewBox(svgPxW, svgPxH) {
  const w = puzzle.width() + 2 * PADDING;
  const h = puzzle.height() + 2 * PADDING;
  const pxPerUnit = Math.min(CELL_PX, svgPxW / w, svgPxH / h);
  const vbW = svgPxW / pxPerUnit;
  const vbH = svgPxH / pxPerUnit;
  const cx = puzzle.width() / 2;
  const cy = puzzle.height() / 2;
  return { x: cx - vbW / 2, y: cy - vbH / 2, w: vbW, h: vbH };
}

// If the canvas aspect changed (e.g. rotation) but the user already pinched,
// rescale the persisted viewBox to the new aspect, keeping its center.
function reconcileViewBoxAspect(svgPxW, svgPxH) {
  if (!viewBoxState) return;
  const canvasAspect = svgPxW / svgPxH;
  const vbAspect = viewBoxState.w / viewBoxState.h;
  if (Math.abs(vbAspect - canvasAspect) <= 1e-3) return;
  const cx = viewBoxState.x + viewBoxState.w / 2;
  const cy = viewBoxState.y + viewBoxState.h / 2;
  let newW = viewBoxState.w;
  let newH = viewBoxState.h;
  if (canvasAspect > vbAspect) newW = newH * canvasAspect;
  else newH = newW / canvasAspect;
  viewBoxState = { x: cx - newW / 2, y: cy - newH / 2, w: newW, h: newH };
}

function renderPuzzle() {
  const w = puzzle.width();
  const h = puzzle.height();
  const { width, height } = svgPixelSize();
  baseViewBox = computeInitialViewBox(width, height);
  reconcileViewBoxAspect(width, height);
  const vb = viewBoxState || baseViewBox;

  const svg = svgEl('svg', {
    viewBox: `${vb.x} ${vb.y} ${vb.w} ${vb.h}`,
    width,
    height,
  });

  svg.appendChild(svgEl('rect', {
    x: -PADDING,
    y: -PADDING,
    width: w + 2 * PADDING,
    height: h + 2 * PADDING,
    fill: BG_INSIDE,
  }));

  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const color = cellColors.get(key(x, y));
      if (color) appendCellColor(svg, x, y, color);
    }
  }

  const badEdges = findBadEdges();
  for (let y = 0; y <= h; y++) {
    for (let x = 0; x < w; x++) {
      appendEdge(svg, x, y, x + 1, y, edgeDisplayState('h', x, y), badEdges.has(edgeKey('h', x, y)));
    }
  }
  for (let y = 0; y < h; y++) {
    for (let x = 0; x <= w; x++) {
      appendEdge(svg, x, y, x, y + 1, edgeDisplayState('v', x, y), badEdges.has(edgeKey('v', x, y)));
    }
  }

  for (let y = 0; y <= h; y++) {
    for (let x = 0; x <= w; x++) {
      svg.appendChild(svgEl('circle', { cx: x, cy: y, r: VERTEX_RADIUS, fill: VERTEX_FILL }));
    }
  }

  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const clue = puzzle.clue(x, y);
      if (clue === undefined) continue;
      const finished = cellLoopCount(x, y) === clue;
      const t = svgEl('text', {
        x: x + 0.5,
        y: y + 0.5,
        'text-anchor': 'middle',
        'dominant-baseline': 'central',
        'font-size': CLUE_FONT_SIZE,
        fill: finished ? CLUE_FILL_FINISHED : CLUE_FILL_UNFINISHED,
      });
      t.textContent = String(clue);
      svg.appendChild(t);
    }
  }

  for (let y = 0; y <= h; y++) {
    for (let x = 0; x < w; x++) {
      appendEdgeHit(svg, x, y, 'h');
    }
  }
  for (let y = 0; y < h; y++) {
    for (let x = 0; x <= w; x++) {
      appendEdgeHit(svg, x, y, 'v');
    }
  }
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      appendCellHit(svg, x, y);
    }
  }

  return svg;
}

function appendCellColor(svg, x, y, color) {
  svg.appendChild(svgEl('rect', {
    x: x,
    y: y,
    width: 1,
    height: 1,
    fill: CELL_COLOR_FILL[color],
  }));
}

function appendEdge(svg, x1, y1, x2, y2, display, bad = false) {
  if (display === DISPLAY_LOOP) {
    svg.appendChild(svgEl('line', {
      x1, y1, x2, y2,
      stroke: bad ? BAD_LOOP_STROKE : LOOP_STROKE,
      'stroke-width': LOOP_WIDTH,
      'stroke-linecap': 'round',
    }));
  } else if (display === DISPLAY_EXCLUDED) {
    svg.appendChild(svgEl('line', {
      x1, y1, x2, y2,
      stroke: SOFT_STROKE,
      'stroke-width': HINT_STROKE_WIDTH,
      'stroke-dasharray': HINT_DASH,
    }));
    const mx = (x1 + x2) / 2;
    const my = (y1 + y2) / 2;
    const r = EXCLUDED_ARM;
    const attrs = { stroke: EXCLUDED_STROKE, 'stroke-width': EXCLUDED_WIDTH, 'stroke-linecap': 'round' };
    svg.appendChild(svgEl('line', { x1: mx - r, y1: my - r, x2: mx + r, y2: my + r, ...attrs }));
    svg.appendChild(svgEl('line', { x1: mx - r, y1: my + r, x2: mx + r, y2: my - r, ...attrs }));
  } else {
    const stroke = display === DISPLAY_SOFT ? SOFT_STROKE : UNSET_STROKE;
    svg.appendChild(svgEl('line', {
      x1, y1, x2, y2,
      stroke,
      'stroke-width': HINT_STROKE_WIDTH,
      'stroke-dasharray': HINT_DASH,
    }));
  }
}

function appendEdgeHit(svg, x, y, axis) {
  const attrs = {
    fill: 'transparent',
    'pointer-events': 'bounding-box',
    'data-type': 'edge',
    'data-axis': axis,
    'data-x': x,
    'data-y': y,
  };
  if (HIT_DEBUG) {
    attrs.stroke = 'red';
    attrs['stroke-width'] = 0.01;
  }
  if (axis === 'h') {
    Object.assign(attrs, {
      x: x + EDGE_HIT_CLIP,
      y: y - EDGE_HIT_WIDTH / 2,
      width: 1 - 2 * EDGE_HIT_CLIP,
      height: EDGE_HIT_WIDTH,
    });
  } else {
    Object.assign(attrs, {
      x: x - EDGE_HIT_WIDTH / 2,
      y: y + EDGE_HIT_CLIP,
      width: EDGE_HIT_WIDTH,
      height: 1 - 2 * EDGE_HIT_CLIP,
    });
  }
  svg.appendChild(svgEl('rect', attrs));
}

function appendCellHit(svg, x, y) {
  const attrs = {
    x: x + CELL_HIT_PAD,
    y: y + CELL_HIT_PAD,
    width: 1 - 2 * CELL_HIT_PAD,
    height: 1 - 2 * CELL_HIT_PAD,
    fill: 'transparent',
    'pointer-events': 'bounding-box',
    'data-type': 'cell',
    'data-x': x,
    'data-y': y,
  };
  if (HIT_DEBUG) {
    attrs.stroke = 'red';
    attrs['stroke-width'] = 0.01;
  }
  svg.appendChild(svgEl('rect', attrs));
}

function cycleColor(current) {
  const i = CELL_COLOR_CYCLE.indexOf(current ?? null);
  return CELL_COLOR_CYCLE[(i + 1) % CELL_COLOR_CYCLE.length];
}

function snapshot() {
  return { userEdges: new Map(userEdges), cellColors: new Map(cellColors) };
}

function applySnapshot(s) {
  userEdges = new Map(s.userEdges);
  cellColors = new Map(s.cellColors);
}

function pushUndo() {
  undoStack.push(snapshot());
  redoStack.length = 0;
}

function doUndo() {
  if (solved || undoStack.length === 0) return;
  redoStack.push(snapshot());
  applySnapshot(undoStack.pop());
  rebuildSolution();
  saveActiveGame();
  render();
}

function doRedo() {
  if (solved || redoStack.length === 0) return;
  undoStack.push(snapshot());
  applySnapshot(redoStack.pop());
  rebuildSolution();
  saveActiveGame();
  render();
}

function doRestart() {
  if (solved) return;
  if (userEdges.size === 0 && cellColors.size === 0) return;
  pushUndo();
  userEdges = new Map();
  cellColors = new Map();
  rebuildSolution();
  saveActiveGame();
  render();
}

function updateActionButtons() {
  // When solved the board is locked: nothing but zoom/pan and Next stays live.
  document.querySelector('[data-action="undo"]').disabled = solved || undoStack.length === 0;
  document.querySelector('[data-action="redo"]').disabled = solved || redoStack.length === 0;
  const restart = document.querySelector('[data-action="restart"]');
  if (restart) restart.disabled = solved;
}

function onClick(ev) {
  if (suppressNextClick) { suppressNextClick = false; return; }
  if (solved) return;  // board is locked once solved
  const t = ev.target;
  const type = t.dataset?.type;
  if (!type) return;
  const x = parseInt(t.dataset.x, 10);
  const y = parseInt(t.dataset.y, 10);
  if (type === 'edge') {
    pushUndo();
    const axis = t.dataset.axis;
    const next = cycleDisplay(edgeDisplayState(axis, x, y));
    const k = edgeKey(axis, x, y);
    if (next === DISPLAY_UNSET) userEdges.delete(k);
    else if (next === DISPLAY_LOOP) userEdges.set(k, 'L');
    else userEdges.set(k, 'X');
    rebuildSolution();
  } else if (type === 'cell') {
    pushUndo();
    const k = key(x, y);
    const next = cycleColor(cellColors.get(k));
    if (next) cellColors.set(k, next);
    else cellColors.delete(k);
  } else {
    return;
  }
  maybeSolve();
  saveActiveGame();
  render();
}

// Lock the board and freeze the timer the first time the puzzle is solved. The
// in-progress board is discarded (a solved puzzle isn't resumable); only a small
// stat record survives.
function maybeSolve() {
  if (solved) return;
  const { isSolved } = window.wasmBindings;
  if (isSolved(puzzle, solution)) {
    solved = true;
    pauseTimer();
    recordSolve();
    removeActiveGame();
  }
}

function render() {
  if (view !== 'board') return;
  const host = document.getElementById('puzzle');
  host.replaceChildren(renderPuzzle());
  host.classList.toggle('solved', solved);
  document.getElementById('puzzle-num').textContent =
    `${diffLabel(currentDiff)} · ${sizeLabel(gridW, gridH)} · #${puzzleNumber}`;
  // Timer and Next only appear once solved; the timer then shows the solve time.
  updateTimerDisplay();
  const timer = document.getElementById('timer');
  timer.hidden = !solved;
  timer.classList.toggle('solved', solved);
  document.getElementById('next-button').hidden = !solved;
  updateActionButtons();
}

function applyViewBox(svg, vb) {
  svg.setAttribute('viewBox', `${vb.x} ${vb.y} ${vb.w} ${vb.h}`);
}

function clampViewBoxToPuzzle(vb) {
  if (!puzzle) return vb;
  const pw = puzzle.width();
  const ph = puzzle.height();
  const m = PAN_VISIBLE_MARGIN;
  const xMin = -vb.w + m;
  const xMax = pw - m;
  const yMin = -vb.h + m;
  const yMax = ph - m;
  const cx = xMin <= xMax ? Math.max(xMin, Math.min(xMax, vb.x)) : (xMin + xMax) / 2;
  const cy = yMin <= yMax ? Math.max(yMin, Math.min(yMax, vb.y)) : (yMin + yMax) / 2;
  return { x: cx, y: cy, w: vb.w, h: vb.h };
}

function commitViewBox(svg, vb) {
  viewBoxState = clampViewBoxToPuzzle(vb);
  applyViewBox(svg, viewBoxState);
}

function readViewBox(svg) {
  const a = svg.getAttribute('viewBox').split(/\s+/).map(Number);
  return { x: a[0], y: a[1], w: a[2], h: a[3] };
}

function screenToPuzzle(svg, sx, sy) {
  const rect = svg.getBoundingClientRect();
  const vb = readViewBox(svg);
  return {
    x: vb.x + (sx - rect.left) / rect.width * vb.w,
    y: vb.y + (sy - rect.top) / rect.height * vb.h,
  };
}

function clampZoomFactor(currentVbW, requestedFactor) {
  if (!baseViewBox) return requestedFactor;
  const currentEff = baseViewBox.w / currentVbW;
  const targetEff = currentEff * requestedFactor;
  const clampedEff = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, targetEff));
  return clampedEff / currentEff;
}

function applyZoomAroundScreenPoint(svg, screenX, screenY, factor) {
  const startVb = readViewBox(svg);
  const f = clampZoomFactor(startVb.w, factor);
  if (f === 1) return;
  const anchor = screenToPuzzle(svg, screenX, screenY);
  const newW = startVb.w / f;
  const newH = startVb.h / f;
  const rect = svg.getBoundingClientRect();
  const newX = anchor.x - (screenX - rect.left) / rect.width * newW;
  const newY = anchor.y - (screenY - rect.top) / rect.height * newH;
  commitViewBox(svg, { x: newX, y: newY, w: newW, h: newH });
}

// Pan helpers — shared between 1-finger touch and mouse drag.
let dragSession = null;
let suppressNextClick = false;

function beginDrag(svg, screenX, screenY) {
  dragSession = {
    svg,
    startX: screenX,
    startY: screenY,
    startVb: readViewBox(svg),
    anchor: screenToPuzzle(svg, screenX, screenY),
    panning: false,
  };
}

function continueDrag(screenX, screenY) {
  if (!dragSession) return false;
  const dx = screenX - dragSession.startX;
  const dy = screenY - dragSession.startY;
  if (!dragSession.panning && Math.hypot(dx, dy) > PAN_THRESHOLD) {
    dragSession.panning = true;
  }
  if (!dragSession.panning) return false;
  const rect = dragSession.svg.getBoundingClientRect();
  const newW = dragSession.startVb.w;
  const newH = dragSession.startVb.h;
  const newX = dragSession.anchor.x - (screenX - rect.left) / rect.width * newW;
  const newY = dragSession.anchor.y - (screenY - rect.top) / rect.height * newH;
  commitViewBox(dragSession.svg, { x: newX, y: newY, w: newW, h: newH });
  return true;
}

function endDrag() {
  if (dragSession && dragSession.panning) suppressNextClick = true;
  dragSession = null;
}

function setupZoom() {
  const container = document.getElementById('puzzle');
  const getSvg = () => container.querySelector('svg');
  let pinch = null;

  container.addEventListener('touchstart', (e) => {
    suppressNextClick = false;
    if (e.touches.length === 2) {
      dragSession = null; // 2 fingers → pinch only
      const svg = getSvg();
      if (!svg) return;
      const t1 = e.touches[0], t2 = e.touches[1];
      const mx = (t1.clientX + t2.clientX) / 2;
      const my = (t1.clientY + t2.clientY) / 2;
      const dist = Math.hypot(t2.clientX - t1.clientX, t2.clientY - t1.clientY);
      pinch = {
        svg,
        startVb: readViewBox(svg),
        anchor: screenToPuzzle(svg, mx, my),
        dist,
      };
      e.preventDefault();
      return;
    }
    if (e.touches.length === 1) {
      const svg = getSvg();
      if (!svg) return;
      const t = e.touches[0];
      beginDrag(svg, t.clientX, t.clientY);
    }
  }, { passive: false });

  container.addEventListener('touchmove', (e) => {
    if (pinch && e.touches.length === 2) {
      const t1 = e.touches[0], t2 = e.touches[1];
      const mx = (t1.clientX + t2.clientX) / 2;
      const my = (t1.clientY + t2.clientY) / 2;
      const newDist = Math.hypot(t2.clientX - t1.clientX, t2.clientY - t1.clientY);
      const f = clampZoomFactor(pinch.startVb.w, newDist / pinch.dist);
      const newW = pinch.startVb.w / f;
      const newH = pinch.startVb.h / f;
      const rect = pinch.svg.getBoundingClientRect();
      const newX = pinch.anchor.x - (mx - rect.left) / rect.width * newW;
      const newY = pinch.anchor.y - (my - rect.top) / rect.height * newH;
      commitViewBox(pinch.svg, { x: newX, y: newY, w: newW, h: newH });
      e.preventDefault();
      return;
    }
    if (dragSession && e.touches.length === 1) {
      const t = e.touches[0];
      if (continueDrag(t.clientX, t.clientY)) e.preventDefault();
    }
  }, { passive: false });

  container.addEventListener('touchend', (e) => {
    if (e.touches.length < 2) pinch = null;
    if (e.touches.length === 0) endDrag();
  });
  container.addEventListener('touchcancel', () => { pinch = null; endDrag(); });

  // Mouse drag for desktop.
  container.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    const svg = getSvg();
    if (!svg) return;
    suppressNextClick = false;
    beginDrag(svg, e.clientX, e.clientY);
  });
  container.addEventListener('mousemove', (e) => { if (dragSession) continueDrag(e.clientX, e.clientY); });
  container.addEventListener('mouseup', endDrag);
  container.addEventListener('mouseleave', endDrag);

  container.addEventListener('wheel', (e) => {
    const svg = getSvg();
    if (!svg) return;
    e.preventDefault();
    const factor = Math.exp(-e.deltaY * WHEEL_SENSITIVITY);
    applyZoomAroundScreenPoint(svg, e.clientX, e.clientY, factor);
  }, { passive: false });
}

// --- Timer ---

function currentElapsed() {
  return elapsedMs + (timerStartedAt !== null ? Date.now() - timerStartedAt : 0);
}

function startTimer() {
  if (solved || timerStartedAt !== null) return;
  timerStartedAt = Date.now();
  timerInterval = setInterval(onTick, 500);
  updateTimerDisplay();
}

// Bank the active segment and stop ticking. Used both when the tab is hidden
// (so background time isn't counted) and permanently when the puzzle is solved.
function pauseTimer() {
  if (timerStartedAt !== null) {
    elapsedMs += Date.now() - timerStartedAt;
    timerStartedAt = null;
  }
  if (timerInterval !== null) {
    clearInterval(timerInterval);
    timerInterval = null;
  }
  updateTimerDisplay();
}

function onTick() {
  updateTimerDisplay();
  saveActiveGame();
}

function formatTime(ms) {
  const total = Math.floor(ms / 1000);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

function updateTimerDisplay() {
  const el = document.getElementById('timer');
  if (el) el.textContent = formatTime(currentElapsed());
}

// --- Category lookups ---

function diffLabel(id) {
  return DIFFICULTIES.find((d) => d.id === id)?.label ?? id;
}

function sizeById(id) {
  return SIZES.find((s) => s.id === id) ?? SIZES[0];
}

function sizeLabel(w, h) {
  return SIZES.find((s) => s.w === w && s.h === h)?.label ?? `${w}x${h}`;
}

function wasmDifficulty(id) {
  const { Difficulty } = window.wasmBindings;
  return id === 'hard' ? Difficulty.Hard : Difficulty.Easy;
}

// Smallest level number not already in progress or solved for this category, so
// "New puzzle" never re-serves a board you've started or finished.
function nextNumber(difficulty, w, h) {
  const used = new Set();
  for (const g of Object.values(activeGames)) {
    if (g.difficulty === difficulty && g.w === w && g.h === h) used.add(g.number);
  }
  for (const s of solves) {
    if (s.difficulty === difficulty && s.w === w && s.h === h) used.add(s.number);
  }
  let n = 1;
  while (used.has(n)) n++;
  return n;
}

// --- Persistence ---

function serializeSnap(s) {
  return { edges: Array.from(s.userEdges.entries()), colors: Array.from(s.cellColors.entries()) };
}

function deserializeSnap(o) {
  return { userEdges: new Map(o?.edges || []), cellColors: new Map(o?.colors || []) };
}

// A game counts as in-progress once the user has done anything that produced
// board state or undo/redo history (so a restarted-to-empty board still counts).
function gameHasState() {
  return userEdges.size > 0 || cellColors.size > 0
    || undoStack.length > 0 || redoStack.length > 0;
}

function saveActiveGame() {
  if (solved || !gameHasState()) return;  // never persist solved or untouched boards
  const id = gameId(currentDiff, gridW, gridH, puzzleNumber);
  activeGames[id] = {
    difficulty: currentDiff,
    w: gridW,
    h: gridH,
    number: puzzleNumber,
    puzzle: currentPuzzleStorage,
    edges: Array.from(userEdges.entries()),
    colors: Array.from(cellColors.entries()),
    undo: undoStack.map(serializeSnap),
    redo: redoStack.map(serializeSnap),
    elapsedMs: currentElapsed(),
    updatedAt: Date.now(),
  };
  writeActive();
}

function removeActiveGame() {
  const id = gameId(currentDiff, gridW, gridH, puzzleNumber);
  if (activeGames[id]) {
    delete activeGames[id];
    writeActive();
  }
}

function recordSolve() {
  solves.push({
    difficulty: currentDiff,
    w: gridW,
    h: gridH,
    number: puzzleNumber,
    timeMs: currentElapsed(),
    solvedAt: Date.now(),
  });
  writeStats();
}

function writeActive() {
  try {
    localStorage.setItem(STORE_ACTIVE, JSON.stringify({ games: activeGames }));
  } catch (e) {
    console.warn('could not save games', e);
  }
}

function writeStats() {
  try {
    localStorage.setItem(STORE_STATS, JSON.stringify({ solves }));
  } catch (e) {
    console.warn('could not save stats', e);
  }
}

// Read + validate both stores. Anything that doesn't parse or match the expected
// shape flips `storageCorrupt`, which surfaces the wipe banner; we keep going
// with empty stores rather than clobbering the bad data automatically.
function readStores() {
  activeGames = readActive();
  solves = readStats();
}

function readActive() {
  const data = parseStore(STORE_ACTIVE);
  if (data === null) return {};
  if (typeof data.games !== 'object' || data.games === null
      || !Object.values(data.games).every(validGame)) {
    storageCorrupt = true;
    return {};
  }
  return data.games;
}

function readStats() {
  const data = parseStore(STORE_STATS);
  if (data === null) return [];
  if (!Array.isArray(data.solves) || !data.solves.every(validSolve)) {
    storageCorrupt = true;
    return [];
  }
  return data.solves;
}

// Returns the parsed object, `null` for an absent key, and flips `storageCorrupt`
// (also returning null) for unreadable JSON.
function parseStore(key) {
  let raw;
  try {
    raw = localStorage.getItem(key);
  } catch (e) {
    return null;
  }
  if (!raw) return null;
  try {
    const data = JSON.parse(raw);
    if (!data || typeof data !== 'object') {
      storageCorrupt = true;
      return null;
    }
    return data;
  } catch (e) {
    storageCorrupt = true;
    return null;
  }
}

function validGame(g) {
  return g && typeof g === 'object'
    && typeof g.difficulty === 'string'
    && Number.isInteger(g.w) && Number.isInteger(g.h) && Number.isInteger(g.number)
    && (g.puzzle === undefined || typeof g.puzzle === 'string')
    && Array.isArray(g.edges) && Array.isArray(g.colors)
    && Array.isArray(g.undo) && Array.isArray(g.redo)
    && typeof g.elapsedMs === 'number';
}

function validSolve(s) {
  return s && typeof s === 'object'
    && typeof s.difficulty === 'string'
    && Number.isInteger(s.w) && Number.isInteger(s.h) && Number.isInteger(s.number)
    && typeof s.timeMs === 'number' && typeof s.solvedAt === 'number';
}

function wipeStorage() {
  try {
    localStorage.removeItem(STORE_ACTIVE);
    localStorage.removeItem(STORE_STATS);
  } catch (e) {
    console.warn('could not wipe storage', e);
  }
  activeGames = {};
  solves = [];
  storageCorrupt = false;
  document.getElementById('wipe-banner').hidden = true;
  if (view === 'home') renderHome();
}

// --- Puzzle loading ---

async function loadGame(difficulty, w, h, number) {
  const { generate, Puzzle } = window.wasmBindings;
  pauseTimer();  // stop the previous puzzle's clock before switching
  const saved = activeGames[gameId(difficulty, w, h, number)];

  // Resume from the stored clue layout when we have one — regenerating reruns
  // the full region+strip pipeline, which is far slower than parsing.
  let next = null;
  if (saved?.puzzle) {
    try {
      next = Puzzle.parse(saved.puzzle);
    } catch (e) {
      console.warn('stored puzzle unparseable, regenerating', e);
    }
  }
  if (!next) {
    // generate() blocks the main thread, so reveal the spinner and let it paint
    // (double rAF) before the call, otherwise the toggle never renders.
    showSpinner(true);
    await nextPaint();
    try {
      next = generate(w, h, wasmDifficulty(difficulty), number);
    } catch (err) {
      showSpinner(false);
      document.getElementById('puzzle').textContent = `Error generating puzzle: ${err}`;
      console.error(err);
      return;
    }
    showSpinner(false);
  }
  if (puzzle) puzzle.free();
  puzzle = next;
  currentPuzzleStorage = next.storage();
  currentDiff = difficulty;
  gridW = w;
  gridH = h;
  puzzleNumber = number;
  userEdges = saved ? new Map(saved.edges) : new Map();
  cellColors = saved ? new Map(saved.colors) : new Map();
  undoStack = saved ? (saved.undo || []).map(deserializeSnap) : [];
  redoStack = saved ? (saved.redo || []).map(deserializeSnap) : [];
  elapsedMs = saved ? saved.elapsedMs : 0;
  timerStartedAt = null;
  viewBoxState = null;  // recenter on the new puzzle

  rebuildSolution();
  solved = false;
  showBoard();
  // Defensive: an active record should never already be solved, but if a restored
  // board satisfies the puzzle, bank it as a solve rather than show a dead board.
  maybeSolve();

  render();
  if (!solved && document.visibilityState !== 'hidden') startTimer();
}

function startNewGame() {
  const size = sizeById(selectedSizeId);
  loadGame(selectedDiff, size.w, size.h, nextNumber(selectedDiff, size.w, size.h));
}

function nextPuzzle() {
  loadGame(currentDiff, gridW, gridH, nextNumber(currentDiff, gridW, gridH));
}

// --- View switching ---

function showSpinner(on) {
  document.getElementById('spinner').hidden = !on;
}

// Resolves after the browser has painted: the first frame applies pending DOM
// changes (e.g. the spinner) and the second guarantees that paint landed before
// a blocking call runs.
function nextPaint() {
  return new Promise((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(resolve));
  });
}

function showBoard() {
  view = 'board';
  document.getElementById('home').hidden = true;
  document.getElementById('topbar').hidden = false;
  document.getElementById('puzzle').hidden = false;
  document.getElementById('action-buttons').hidden = false;
}

function showHome() {
  // Bank the current board (if any) before leaving, then return to the menu.
  if (view === 'board') {
    pauseTimer();
    saveActiveGame();
  }
  view = 'home';
  document.getElementById('topbar').hidden = true;
  document.getElementById('puzzle').hidden = true;
  document.getElementById('action-buttons').hidden = true;
  document.getElementById('home').hidden = false;
  renderHome();
}

// --- Home screen ---

function renderHome() {
  renderPicker('diff-picker', DIFFICULTIES, selectedDiff, (id) => {
    selectedDiff = id;
    renderHome();
  });
  renderPicker('size-picker', SIZES, selectedSizeId, (id) => {
    selectedSizeId = id;
    renderHome();
  });
  renderResumeList();
  renderStats();
}

function renderPicker(containerId, items, selectedId, onSelect) {
  const c = document.getElementById(containerId);
  c.replaceChildren();
  for (const it of items) {
    const b = document.createElement('button');
    b.type = 'button';
    b.className = 'pill' + (it.id === selectedId ? ' selected' : '');
    b.textContent = it.label;
    b.addEventListener('click', () => onSelect(it.id));
    c.appendChild(b);
  }
}

function renderResumeList() {
  const list = document.getElementById('resume-list');
  list.replaceChildren();
  const games = Object.values(activeGames).sort((a, b) => (b.updatedAt || 0) - (a.updatedAt || 0));
  if (games.length === 0) {
    const li = document.createElement('li');
    li.className = 'home-empty';
    li.textContent = 'No puzzles in progress.';
    list.appendChild(li);
    return;
  }
  for (const g of games) {
    const li = document.createElement('li');
    li.className = 'game-item';
    const main = document.createElement('span');
    main.className = 'game-main';
    main.textContent = `${diffLabel(g.difficulty)} · ${sizeLabel(g.w, g.h)} · #${g.number}`;
    const time = document.createElement('span');
    time.className = 'game-time';
    time.textContent = formatTime(g.elapsedMs || 0);
    li.append(main, time);
    li.addEventListener('click', () => loadGame(g.difficulty, g.w, g.h, g.number));
    list.appendChild(li);
  }
}

function renderStats() {
  const host = document.getElementById('stats-summary');
  host.replaceChildren();
  if (solves.length === 0) {
    host.className = 'home-empty';
    host.textContent = 'No puzzles solved yet.';
    return;
  }
  host.className = '';
  const byCat = new Map();
  for (const s of solves) {
    const k = `${s.difficulty}:${s.w}x${s.h}`;
    let agg = byCat.get(k);
    if (!agg) {
      agg = { difficulty: s.difficulty, w: s.w, h: s.h, count: 0, best: Infinity };
      byCat.set(k, agg);
    }
    agg.count++;
    agg.best = Math.min(agg.best, s.timeMs);
  }
  const ul = document.createElement('ul');
  ul.className = 'home-list';
  for (const agg of byCat.values()) {
    const li = document.createElement('li');
    li.className = 'stat-item';
    const main = document.createElement('span');
    main.className = 'game-main';
    main.textContent = `${diffLabel(agg.difficulty)} · ${sizeLabel(agg.w, agg.h)}`;
    const detail = document.createElement('span');
    detail.className = 'game-time';
    detail.textContent = `${agg.count} solved · best ${formatTime(agg.best)}`;
    li.append(main, detail);
    ul.appendChild(li);
  }
  host.appendChild(ul);
}

function setupMenu() {
  const btn = document.getElementById('menu-button');
  const menu = document.getElementById('menu');
  const setOpen = (open) => {
    menu.hidden = !open;
    btn.setAttribute('aria-expanded', String(open));
  };
  btn.addEventListener('click', (e) => {
    e.stopPropagation();
    setOpen(menu.hidden);
  });
  document.addEventListener('click', (e) => {
    if (menu.hidden) return;
    if (!menu.contains(e.target) && e.target !== btn) setOpen(false);
  });
  menu.addEventListener('click', (e) => {
    const action = e.target.closest('[data-action]')?.dataset.action;
    if (action === 'restart') doRestart();
    setOpen(false);
  });
}

function init() {
  readStores();
  if (storageCorrupt) document.getElementById('wipe-banner').hidden = false;
  document.getElementById('wipe-button').addEventListener('click', wipeStorage);

  setupMenu();
  setupZoom();
  const host = document.getElementById('puzzle');
  host.style.background = BG_OUTSIDE;
  host.addEventListener('click', onClick);
  document.getElementById('action-buttons').addEventListener('click', (e) => {
    const action = e.target.closest('[data-action]')?.dataset.action;
    if (action === 'undo') doUndo();
    else if (action === 'redo') doRedo();
  });
  document.getElementById('next-button').addEventListener('click', nextPuzzle);
  document.getElementById('home-button').addEventListener('click', showHome);
  document.getElementById('new-game').addEventListener('click', startNewGame);
  window.addEventListener('resize', () => { if (view === 'board') render(); });

  // Don't count time while the tab is in the background; resume when it returns.
  document.addEventListener('visibilitychange', () => {
    if (view !== 'board') return;
    if (document.hidden) {
      pauseTimer();
      saveActiveGame();
    } else if (!solved) {
      startTimer();
    }
  });
  window.addEventListener('pagehide', () => {
    if (view !== 'board') return;
    pauseTimer();
    saveActiveGame();
  });

  showHome();
}

if (window.wasmBindings) {
  init();
} else {
  window.addEventListener('TrunkApplicationStarted', init, { once: true });
}
