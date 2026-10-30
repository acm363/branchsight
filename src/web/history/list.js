import { $, drag } from '../shared/dom.js';
import { SHORT_HASH, clamp, color, esc, fmtDate } from '../shared/format.js';
import { refLabels } from './labels.js';
import { prefs } from '../shared/prefs.js';
import { isWorktree, state } from '../shared/state.js';

const AUTO_LANES = 24;
const COLUMN_LIMITS = { graph: [30, 640], author: [40, 400], date: [60, 220], hash: [40, 200] };
const TOGGLE_COLS = [['Date', 'date'], ['Author', 'author'], ['Commit', 'hash']];
const ICONS = {
  branch: '<svg viewBox="0 0 16 16"><path d="M5 3.5a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0Zm0 9a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0Zm9-6a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0ZM3.5 5v6M12.5 8c0 2-2 3-4 3H3.5" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>',
  tag: '<svg viewBox="0 0 16 16"><path d="M2 2.5h5.6l6.4 6.4-5.1 5.1L2.5 7.6V2.5Z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/><circle cx="5.2" cy="5.7" r="1.1" fill="currentColor"/></svg>',
};
const LABEL_CLASS = { head: 'local', detached: 'local', remote: 'remote-only', tag: 'tag' };

let ROW = 21, LANE = 17;
let columns = prefs.get('cols') || {};
let painted = { first: -1, last: -1 };

export const rowHeight = () => ROW;

export const entryKey = entry => entry && (isWorktree(entry) ? 'worktree' : entry.h);

export function labelsOf(commit) {
  const { branch, head } = state.repo;
  return refLabels(state.refsByHash.get(commit.h) || [], { branch, detached: commit.h === head && !branch });
}

export function setRowHeight(row) {
  ROW = row;
  LANE = Math.round(row * 0.8);
  document.documentElement.style.setProperty('--row', `${row}px`);
  sizeRows();
  render();
}

function sizeRows() {
  $('spacer').style.height = `${state.entries.length * ROW}px`;
  applyColumns();
}

const graphWidth = () => columns.graph || Math.round(LANE * 1.1) + Math.min(state.lanes, AUTO_LANES) * LANE;

function applyColumns() {
  const pane = $('pane');
  ['author', 'date', 'hash'].forEach(k => columns[k]
    ? pane.style.setProperty(`--w-${k}`, `${columns[k]}px`)
    : pane.style.removeProperty(`--w-${k}`));
  pane.style.setProperty('--w-graph', `${graphWidth()}px`);
}

const colShow = () => ({ author: true, date: true, hash: true, ...(prefs.get('colShow') || {}) });

export const columnMenuItems = () => TOGGLE_COLS.map(([label, name]) => [label, name, colShow()[name]]);

export function toggleColumn(name) {
  const show = colShow();
  prefs.set('colShow', { ...show, [name]: !show[name] });
  applyColumnVisibility();
}

function applyColumnVisibility() {
  const show = colShow();
  TOGGLE_COLS.forEach(([, name]) => $('pane').classList.toggle(`hide-${name}`, !show[name]));
}

export function show({ entries, rows, lanes }, { cursorKey = null, top = null } = {}) {
  Object.assign(state, { entries, rows, lanes });
  state.cursor = cursorKey === null ? -1 : entries.findIndex(e => entryKey(e) === cursorKey);
  sizeRows();
  const list = $('list');
  const topAgain = top && !top.atTop ? entries.findIndex(e => entryKey(e) === top.key) : -1;
  if (top?.atTop) list.scrollTop = 0;
  else if (topAgain >= 0) list.scrollTop = topAgain * ROW + top.offset;
  else if (state.cursor < 0) list.scrollTop = 0;
  render();
}

export function anchor() {
  const top = $('list').scrollTop, i = Math.floor(top / ROW);
  return { key: entryKey(state.entries[i]), offset: top - i * ROW, atTop: top < ROW };
}

export function setCursor(i, { scroll = 'nearest' } = {}) {
  if (!state.entries.length) return;
  state.cursor = clamp(i, 0, state.entries.length - 1);
  const list = $('list'), y = state.cursor * ROW;
  if (scroll === 'center') list.scrollTop = Math.max(0, y - list.clientHeight / 2 + ROW / 2);
  else if (scroll && y < list.scrollTop) list.scrollTop = y;
  else if (scroll && y + ROW > list.scrollTop + list.clientHeight) list.scrollTop = y + ROW - list.clientHeight;
  render();
}

export function focus(hash) {
  const i = state.entries.findIndex(e => e.h === hash);
  if (i >= 0) setCursor(i, { scroll: 'center' });
}

function visibleRange() {
  const list = $('list');
  return {
    first: Math.max(0, Math.floor(list.scrollTop / ROW) - 4),
    last: Math.min(state.entries.length, Math.ceil((list.scrollTop + list.clientHeight) / ROW) + 4),
  };
}

export function render() {
  const { first, last } = painted = visibleRange();
  let html = '';
  for (let i = first; i < last; i++) html += rowHtml(state.entries[i], i);
  $('rows').innerHTML = html;
  draw(first, last, graphWidth());
}

function renderIfMoved() {
  const { first, last } = visibleRange();
  if (first !== painted.first || last !== painted.last) render();
}

const worktreeSummary = status => [[status.conflicts, 'in conflict'], [status.staged, 'staged'], [status.unstaged, 'modified'], [status.untracked, 'untracked']]
  .filter(([n]) => n).map(([n, what]) => `${n} ${what}`).join(' · ');

function rowHtml(entry, i) {
  const cls = [
    i === state.cursor && 'sel',
    entry.h && entry.h === state.compareHash && 'cmp',
    entry.p.length > 1 && 'merge',
    isWorktree(entry) && 'wt',
  ].filter(Boolean).map(c => ` ${c}`).join('');
  const open = `<div class="row${cls}" data-i="${i}" style="top:${i * ROW}px">`;
  if (isWorktree(entry)) {
    return `${open}<span class="s"><span class="t">Uncommitted changes</span><span class="wtc">${esc(worktreeSummary(entry.status))}</span></span>`
      + '<span class="a"></span><span class="d"></span><span class="h"></span></div>';
  }
  return `${open}<span class="s">${labelsHtml(entry, color(state.rows[i].color))}<span class="t">${esc(entry.s)}</span></span>`
    + `<span class="a">${esc(entry.a)}</span><span class="d">${fmtDate(entry.t)}</span>`
    + `<span class="h" title="Copy ${entry.h}">${entry.h.slice(0, SHORT_HASH)}</span></div>`;
}

function labelsHtml(commit, laneColor) {
  return labelsOf(commit).map(label => {
    const cls = `${LABEL_CLASS[label.kind]}${label.current ? ' current' : ''}`;
    const remotes = label.kind === 'head' ? label.remotes.map(r => `<span class="remote">${esc(r.remote)}</span>`).join('') : '';
    return `<span class="ref ${cls}" style="--c:${laneColor}" data-kind="${label.kind}" data-name="${esc(label.name)}">`
      + `${ICONS[label.kind === 'tag' ? 'tag' : 'branch']}<span class="name">${esc(label.name)}</span>${remotes}</span>`;
  }).join('');
}

function draw(first, last, width) {
  const canvas = $('graph'), dpr = window.devicePixelRatio || 1, height = Math.max(1, (last - first) * ROW);
  canvas.style.top = `${first * ROW}px`;
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;
  const g = canvas.getContext('2d');
  const [pixelsWide, pixelsHigh] = [Math.round(width * dpr), Math.round(height * dpr)];
  if (canvas.width !== pixelsWide || canvas.height !== pixelsHigh) {
    canvas.width = pixelsWide;
    canvas.height = pixelsHigh;
  } else {
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.clearRect(0, 0, canvas.width, canvas.height);
  }
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.lineWidth = ROW * 0.12;
  g.lineCap = 'round';
  const background = getComputedStyle(document.body).backgroundColor;
  const dim = getComputedStyle(document.documentElement).getPropertyValue('--dim');
  const X = lane => Math.round(LANE * 0.55) + lane * LANE;
  const line = (x1, y1, x2, y2, stroke, dashed) => {
    if (Math.min(x1, x2) > width) return;
    g.strokeStyle = stroke;
    g.setLineDash(dashed ? [2, 3] : []);
    g.beginPath();
    g.moveTo(x1, y1);
    if (x1 === x2) g.lineTo(x2, y2);
    else {
      const m = (y1 + y2) / 2;
      g.bezierCurveTo(x1, m, x2, m, x2, y2);
    }
    g.stroke();
  };
  const dot = (x, y, radius, fill, stroke, dashed) => {
    g.setLineDash(dashed ? [2, 2] : []);
    g.beginPath();
    g.arc(x, y, radius, 0, Math.PI * 2);
    g.fillStyle = fill;
    g.strokeStyle = stroke;
    g.fill();
    g.stroke();
  };
  for (let i = first; i < last; i++) {
    const r = state.rows[i], entry = state.entries[i], wt = isWorktree(entry);
    const top = (i - first) * ROW, mid = top + ROW / 2, bottom = top + ROW, x = X(r.col);
    r.before.forEach((hash, lane) => {
      if (hash == null) return;
      if (hash === entry.h) line(X(lane), top, x, mid, color(r.bc[lane]));
      else line(X(lane), top, X(lane), bottom, color(r.bc[lane]));
    });
    r.out.forEach(([lane, n, known]) => line(x, mid, X(lane), bottom, wt ? dim : color(n), wt || !known));
    if (x > width) continue;
    const merge = entry.p.length > 1, node = color(r.color), head = !wt && entry.h === state.repo.head;
    if (wt) dot(x, mid, ROW * 0.18, background, dim, true);
    else if (head) dot(x, mid, ROW * 0.24, background, node);
    else dot(x, mid, ROW * (merge ? 0.15 : 0.18), merge ? background : node, node);
  }
}

applyColumnVisibility();

$('list').addEventListener('scroll', () => requestAnimationFrame(renderIfMoved), { passive: true });
window.addEventListener('resize', () => requestAnimationFrame(render));

$('cols').addEventListener('mousedown', e => {
  const grip = e.target.closest('.grip');
  if (!grip) return;
  const name = grip.dataset.col, direction = +grip.dataset.dir, x0 = e.clientX;
  const start = name === 'graph' ? graphWidth() : parseFloat(getComputedStyle($('pane')).getPropertyValue(`--w-${name}`));
  const [min, max] = COLUMN_LIMITS[name];
  drag(e, ev => {
    columns[name] = Math.round(clamp(start + direction * (ev.clientX - x0), min, max));
    applyColumns();
    render();
  }, () => prefs.set('cols', columns));
});

$('cols').addEventListener('dblclick', e => {
  const grip = e.target.closest('.grip');
  if (!grip) return;
  delete columns[grip.dataset.col];
  prefs.set('cols', columns);
  applyColumns();
  render();
});
