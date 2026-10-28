import { getJson } from '../shared/api.js';
import { buildDiff, folds } from './diffview.js';
import { $, closeLayer, drag, openLayer } from '../shared/dom.js';
import { clamp, esc, fileLabel, plural } from '../shared/format.js';
import { prefs } from '../shared/prefs.js';

const SPLIT_LIMITS = [0.15, 0.85];
const GUTTER_EM = 4.2;
const SIGNS = { del: '−', add: '+', ctx: ' ' };

let viewer = null;
let split = 0.5;
let geometry = null;
let find = { q: '', matches: [], current: -1 };

const layer = { contains: () => true, close: hide, keydown };

export function open(source, files, index) {
  if (!files.length) return;
  viewer = { source, files, index, mode: prefs.get('diffMode') === 'unified' ? 'unified' : 'split', change: 0, data: null };
  $('v-file').innerHTML = files.map((f, i) => `<option value="${i}">${f.status}  ${esc(fileLabel(f))}</option>`).join('');
  $('viewer').hidden = false;
  openLayer(layer);
  setSplit(prefs.get('diffSplit') ?? 0.5);
  load();
}

function hide() {
  viewer = null;
  find = { q: '', matches: [], current: -1 };
  $('v-find').value = '';
  updateFind();
  $('viewer').hidden = true;
  $('list').focus();
}

function keydown(e) {
  if ((e.ctrlKey || e.metaKey) && (e.key === 'f' || e.key === 'F')) {
    $('v-find').focus();
    $('v-find').select();
    e.preventDefault();
    return true;
  }
  if (e.key === 'Escape') {
    if (document.activeElement === $('v-find') && find.q) {
      setFind('');
      return true;
    }
    return false;
  }
  if (e.ctrlKey || e.metaKey || e.altKey || e.target.closest?.('input, select')) return true;
  const action = {
    n: () => step(1), p: () => step(-1), N: () => step(-1), F7: () => step(e.shiftKey ? -1 : 1),
    ']': () => moveFile(1), '[': () => moveFile(-1),
  }[e.key];
  if (action) {
    action();
    e.preventDefault();
  }
  return true;
}

function diffParams(source, file) {
  const params = new URLSearchParams({ kind: file.kind });
  if (source.kind === 'commit') params.set('h', source.h);
  if (source.kind === 'compare') {
    params.set('from', source.from);
    params.set('to', source.to);
  }
  if (file.from) params.append('path', file.from);
  params.append('path', file.path);
  return params;
}

async function load(atEnd = false) {
  const v = viewer, file = v.files[v.index];
  const [from, to] = file.sides;
  $('v-from').textContent = from;
  $('v-to').textContent = to;
  $('v-file').value = String(v.index);
  $('v-prev-file').disabled = v.index === 0;
  $('v-next-file').disabled = v.index === v.files.length - 1;
  $('v-pos').textContent = '';
  $('v-cut').hidden = true;
  $('v-body').innerHTML = '<div class="v-empty">loading…</div>';
  v.data = null;
  v.failed = false;
  v.open = new Set();
  let diff;
  try {
    diff = await getJson('/api/filediff', diffParams(v.source, file));
  } catch (e) {
    if (viewer !== v || v.files[v.index] !== file) return;
    v.failed = true;
    $('v-body').innerHTML = `<div class="v-empty">${esc(e.message)}</div>`;
    return;
  }
  if (viewer !== v || v.files[v.index] !== file) return;
  $('v-cut').hidden = !diff.truncated;
  v.data = buildDiff(diff.patch);
  v.change = atEnd ? Math.max(0, v.data.changes.length - 1) : 0;
  render();
  showChange();
}

function codeCell(cell, sign = '') {
  if (!cell) return '<span class="code none"></span>';
  const { text, range: r } = cell;
  const inner = r && r[1] > r[0] ? `${esc(text.slice(0, r[0]))}<mark>${esc(text.slice(r[0], r[1]))}</mark>${esc(text.slice(r[1]))}` : esc(text);
  return `<span class="code ${cell.kind}">${sign}${inner || (sign ? '' : ' ')}</span>`;
}

function render() {
  const v = viewer, d = v.data;
  $('viewer').classList.toggle('unified', v.mode === 'unified');
  $('v-mode').querySelectorAll('button').forEach(b => b.classList.toggle('on', b.dataset.mode === v.mode));
  if (d.binary) {
    $('v-body').innerHTML = '<div class="v-empty">Binary file: no text to compare.</div>';
    return;
  }
  const starts = new Map(d.changes.map((c, k) => [v.mode === 'split' ? c.split : c.unified, k]));
  const tag = i => starts.has(i) ? ` data-change="${starts.get(i)}"` : '';
  const split = v.mode === 'split', list = split ? d.split : d.unified;
  v.rows = split
    ? list.map((r, i) => r.gap ? '<div class="v-row gap">⋯</div>'
      : `<div class="v-row${r.change ? ' chg' : ''}"${tag(i)}><span class="ln">${r.left?.n ?? ''}</span>${codeCell(r.left)}<span class="ln">${r.right?.n ?? ''}</span>${codeCell(r.right)}</div>`)
    : list.map((r, i) => r.gap ? '<div class="v-row u gap">⋯</div>'
      : `<div class="v-row u${r.kind !== 'ctx' ? ' chg' : ''}"${tag(i)}><span class="ln">${r.old ?? ''}</span><span class="ln">${r.new ?? ''}</span>${codeCell(r, SIGNS[r.kind])}</div>`);
  const lineOf = r => split ? r.right.n : r.new;
  const rows = [];
  let next = 0;
  for (const { from, to } of folds(list.map(r => !r.gap && (split ? !r.change : r.kind === 'ctx')))) {
    const key = lineOf(list[from]);
    if (v.open.has(key)) continue;
    rows.push(...v.rows.slice(next, from), `<div class="v-row unfold" data-key="${key}" data-from="${from}" data-to="${to}">⇕ ${plural(to - from, 'unmodified line')}</div>`);
    next = to;
  }
  rows.push(...v.rows.slice(next));
  $('v-body').innerHTML = rows.join('') || '<div class="v-empty">No change in this file.</div>';
  $('v-body').scrollTop = 0;
  geometry = null;
  placeDivider();
  if (find.q) applyFind(true);
}

function step(direction) {
  const v = viewer, next = v.change + direction;
  if (!v.data) {
    if (v.failed) moveFile(direction, direction < 0);
    return;
  }
  if (next >= 0 && next < v.data.changes.length) {
    v.change = next;
    showChange();
  } else moveFile(direction, direction < 0);
}

function showChange() {
  const v = viewer, total = v.data.changes.length, body = $('v-body');
  $('v-pos').textContent = total ? `change ${v.change + 1} of ${total}` : 'no change';
  body.querySelectorAll('.cur').forEach(el => el.classList.remove('cur'));
  let row = body.querySelector(`[data-change="${v.change}"]`);
  if (!row) return;
  const target = row;
  target.scrollIntoView({ block: 'center' });
  requestAnimationFrame(() => target.isConnected && target.scrollIntoView({ block: 'center' }));
  for (; row?.classList.contains('chg'); row = row.nextElementSibling) row.classList.add('cur');
}

function moveFile(step, atEnd = false) {
  const v = viewer, next = v.index + step;
  if (next < 0 || next >= v.files.length) return;
  v.index = next;
  load(atEnd);
}

function setFind(value) {
  find.q = value;
  $('v-find').value = value;
  applyFind(false);
}

function applyFind(keep) {
  const body = $('v-body');
  body.querySelectorAll('mark.find').forEach(m => m.replaceWith(document.createTextNode(m.textContent)));
  body.normalize();
  const needle = find.q.toLowerCase();
  if (needle) {
    const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT);
    const nodes = [];
    for (let n = walker.nextNode(); n; n = walker.nextNode()) {
      if (n.nodeValue.toLowerCase().includes(needle)) nodes.push(n);
    }
    nodes.forEach(n => wrapMatches(n, needle));
  }
  find.matches = needle ? [...body.querySelectorAll('mark.find')] : [];
  find.current = find.matches.length ? clamp(keep && find.current >= 0 ? find.current : 0, 0, find.matches.length - 1) : -1;
  showMatch(!keep);
  updateFind();
}

function wrapMatches(node, needle) {
  const text = node.nodeValue, lower = text.toLowerCase(), frag = document.createDocumentFragment();
  let i = 0, at;
  while ((at = lower.indexOf(needle, i)) >= 0) {
    if (at > i) frag.append(text.slice(i, at));
    const mark = document.createElement('mark');
    mark.className = 'find';
    mark.textContent = text.slice(at, at + needle.length);
    frag.append(mark);
    i = at + needle.length;
  }
  if (i < text.length) frag.append(text.slice(i));
  node.replaceWith(frag);
}

function showMatch(scroll) {
  find.matches.forEach((m, i) => m.classList.toggle('on', i === find.current));
  if (scroll && find.current >= 0) find.matches[find.current].scrollIntoView({ block: 'center' });
}

function stepFind(direction) {
  if (!find.matches.length) return;
  find.current = (find.current + direction + find.matches.length) % find.matches.length;
  showMatch(true);
  updateFind();
}

function updateFind() {
  const total = find.matches.length;
  $('v-find-count').textContent = find.q ? `${total ? find.current + 1 : 0}/${total}` : '';
  $('v-find-prev').disabled = $('v-find-next').disabled = !total;
}

function measure() {
  const body = $('v-body');
  const gutter = parseFloat(getComputedStyle(body).fontSize) * GUTTER_EM;
  geometry = { gutter, usable: body.clientWidth - 2 * gutter, left: body.getBoundingClientRect().left, top: $('v-bar').offsetHeight };
}

function placeDivider() {
  if (!geometry) measure();
  $('v-divider').style.left = `${geometry.gutter + split * geometry.usable}px`;
  $('v-divider').style.top = `${geometry.top}px`;
}

function setSplit(fraction) {
  split = clamp(fraction, ...SPLIT_LIMITS);
  $('viewer').style.setProperty('--v-l', `${split}fr`);
  $('viewer').style.setProperty('--v-r', `${1 - split}fr`);
  placeDivider();
}

$('v-close').addEventListener('click', () => closeLayer(layer));
$('v-prev-file').addEventListener('click', () => moveFile(-1));
$('v-next-file').addEventListener('click', () => moveFile(1));
$('v-prev').addEventListener('click', () => step(-1));
$('v-next').addEventListener('click', () => step(1));

let findTimer = 0;
$('v-find').addEventListener('input', e => {
  find.q = e.target.value;
  clearTimeout(findTimer);
  findTimer = setTimeout(() => applyFind(false), 100);
});
$('v-find').addEventListener('keydown', e => {
  if (e.key === 'Enter') {
    e.preventDefault();
    stepFind(e.shiftKey ? -1 : 1);
  }
});
$('v-find-prev').addEventListener('click', () => stepFind(-1));
$('v-find-next').addEventListener('click', () => stepFind(1));

$('v-body').addEventListener('click', e => {
  const fold = e.target.closest('.unfold');
  if (!fold) return;
  viewer.open.add(+fold.dataset.key);
  fold.outerHTML = viewer.rows.slice(+fold.dataset.from, +fold.dataset.to).join('');
  if (find.q) applyFind(true);
});

$('v-file').addEventListener('change', e => {
  viewer.index = +e.target.value;
  load();
});

$('v-mode').addEventListener('click', e => {
  const mode = e.target.closest('[data-mode]')?.dataset.mode;
  if (!mode || !viewer?.data) return;
  viewer.mode = mode;
  prefs.set('diffMode', mode);
  render();
  showChange();
});

$('v-divider').addEventListener('mousedown', e => {
  const divider = e.currentTarget;
  measure();
  divider.classList.add('drag');
  drag(e, ev => setSplit((ev.clientX - geometry.left - geometry.gutter) / geometry.usable), () => {
    divider.classList.remove('drag');
    prefs.set('diffSplit', split);
  });
});

$('v-divider').addEventListener('dblclick', () => {
  setSplit(0.5);
  prefs.set('diffSplit', split);
});

window.addEventListener('resize', () => {
  geometry = null;
  if (viewer) placeDivider();
});
