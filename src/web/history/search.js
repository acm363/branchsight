import { $ } from '../shared/dom.js';
import { esc, fmtDate } from '../shared/format.js';
import { prefs } from '../shared/prefs.js';
import { isWorktree, state } from '../shared/state.js';
import { labelsOf, onShown, render, setCursor, setSearch } from './list.js';
import { buildMatcher } from './match.js';

let q = '';
let caseSensitive = prefs.get('searchCase') ?? false;
let regex = prefs.get('searchRegex') ?? false;
let matcher = null;
let matches = [];
const matchSet = new Set();
let current = -1;
let open = false;
let timer = 0;

function build() {
  matcher = buildMatcher(q, { caseSensitive, regex });
  $('s-find').classList.toggle('bad', !!matcher?.invalid);
}

function haystack(entry) {
  const labels = labelsOf(entry).flatMap(l => [l.name, ...l.remotes.map(r => r.remote)]);
  return [entry.s, entry.a, fmtDate(entry.t), entry.h, ...labels].join('\n');
}

function highlight(text) {
  const ranges = matcher && text ? matcher.ranges(text) : [];
  if (!ranges.length) return esc(text);
  let out = '', i = 0;
  for (const [a, b] of ranges) {
    out += `${esc(text.slice(i, a))}<mark class="hit">${esc(text.slice(a, b))}</mark>`;
    i = b;
  }
  return out + esc(text.slice(i));
}

function recompute(keep) {
  build();
  matches = [];
  matchSet.clear();
  if (matcher) {
    for (let i = 0; i < state.entries.length; i++) {
      const entry = state.entries[i];
      if (!isWorktree(entry) && matcher.test(haystack(entry))) {
        matches.push(i);
        matchSet.add(i);
      }
    }
  }
  current = matches.length ? (keep && current >= 0 ? Math.min(current, matches.length - 1) : 0) : -1;
  updateCount();
}

function jump() {
  setCursor(matches[current], { scroll: 'center' });
  updateCount();
}

function updateCount() {
  const total = matches.length;
  $('s-count').textContent = q ? `${total ? current + 1 : 0}/${total}` : '';
  $('s-prev').disabled = $('s-next').disabled = !total;
}

function step(direction) {
  if (!matches.length) return;
  current = (current + direction + matches.length) % matches.length;
  jump();
}

export const isOpen = () => open;

export function openSearch() {
  open = true;
  $('search').hidden = false;
  $('s-find').focus();
  $('s-find').select();
  recompute(true);
  render();
}

export function close() {
  open = false;
  clearTimeout(timer);
  $('search').hidden = true;
  matcher = null;
  matches = [];
  matchSet.clear();
  current = -1;
  render();
  $('list').focus();
}

function markToggles() {
  $('s-case').classList.toggle('on', caseSensitive);
  $('s-regex').classList.toggle('on', regex);
}

function toggle(which) {
  if (which === 'case') prefs.set('searchCase', (caseSensitive = !caseSensitive));
  else prefs.set('searchRegex', (regex = !regex));
  markToggles();
  recompute(true);
  render();
}

setSearch({ mark: highlight, rowMatch: i => matchSet.has(i) });
onShown(() => open && recompute(true));

$('s-find').addEventListener('input', e => {
  q = e.target.value;
  clearTimeout(timer);
  timer = setTimeout(() => {
    recompute(false);
    current >= 0 ? jump() : render();
  }, 100);
});
$('s-find').addEventListener('keydown', e => {
  if (e.key === 'Enter') {
    e.preventDefault();
    step(e.shiftKey ? -1 : 1);
  } else if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    close();
  }
});
$('s-prev').addEventListener('click', () => step(-1));
$('s-next').addEventListener('click', () => step(1));
$('s-case').addEventListener('click', () => toggle('case'));
$('s-regex').addEventListener('click', () => toggle('regex'));
$('s-close').addEventListener('click', close);
$('search-btn').addEventListener('click', () => (open ? close() : openSearch()));

markToggles();
