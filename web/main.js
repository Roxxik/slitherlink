const PUZZLE_URL = 'puzzles/simple-7x7.txt';

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
  if (undoStack.length === 0) return;
  redoStack.push(snapshot());
  applySnapshot(undoStack.pop());
  rebuildSolution();
  render();
}

function doRedo() {
  if (redoStack.length === 0) return;
  undoStack.push(snapshot());
  applySnapshot(redoStack.pop());
  rebuildSolution();
  render();
}

function doRestart() {
  if (userEdges.size === 0 && cellColors.size === 0) return;
  pushUndo();
  userEdges = new Map();
  cellColors = new Map();
  rebuildSolution();
  render();
}

function updateActionButtons() {
  document.querySelector('[data-action="undo"]').disabled = undoStack.length === 0;
  document.querySelector('[data-action="redo"]').disabled = redoStack.length === 0;
}

function onClick(ev) {
  if (suppressNextClick) { suppressNextClick = false; return; }
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
  }
  render();
}

function setStatus(text, kind = '') {
  const el = document.getElementById('status');
  el.textContent = text;
  el.className = kind;
}

function render() {
  const { isSolved } = window.wasmBindings;
  const host = document.getElementById('puzzle');
  host.replaceChildren(renderPuzzle());
  const solved = isSolved(puzzle, solution);
  setStatus(solved ? 'solved' : 'unsolved', solved ? 'ok' : '');
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

async function init() {
  const { Puzzle } = window.wasmBindings;
  try {
    const response = await fetch(PUZZLE_URL);
    if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
    const puzzleStr = await response.text();
    puzzle = Puzzle.parse(puzzleStr);
    userEdges = new Map();
    cellColors = new Map();
    rebuildSolution();
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
    window.addEventListener('resize', render);
    render();
  } catch (err) {
    setStatus(`Error loading ${PUZZLE_URL}: ${err}`, 'bad');
    console.error(err);
  }
}

if (window.wasmBindings) {
  init();
} else {
  window.addEventListener('TrunkApplicationStarted', init, { once: true });
}
