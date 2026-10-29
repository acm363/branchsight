import { $ } from '../shared/dom.js';
import { esc, fmtDate, fuzzy, highlight } from '../shared/format.js';
import { picker } from '../shared/picker.js';
import { state } from '../shared/state.js';
import { loadLog, onRefs } from '../history/sync.js';

const MAX_SHOWN = 400, MAX_CHIPS = 5;
const GROUPS = [
  { name: '@local', label: 'all local branches', kind: 'head' },
  { name: '@remotes', label: 'all remote branches', kind: 'remote' },
  { name: '@tags', label: 'all tags', kind: 'tag' },
  { name: 'HEAD', label: 'HEAD', kind: 'head' },
];

export const DEFAULT_SELECTION = ['@local', '@remotes', '@tags'];

const menu = picker({
  anchor: $('picker'),
  panel: $('menu'),
  input: $('filter'),
  list: $('reflist'),
  tabChooses: true,
  items: matching,
  row: ({ ref, match }, i, current) => {
    const on = state.selected.includes(ref.name);
    return `<div class="pick k-${ref.kind}${ref.group ? ' group' : ''}${on ? ' on' : ''}${current ? ' cur' : ''}" data-i="${i}">`
      + `<span class="box">${on ? '✓' : ''}</span><span class="name">${highlight(ref.label || ref.name, match.marks)}</span>`
      + `<span class="when">${ref.date ? fmtDate(ref.date).slice(0, 10) : ''}</span></div>`;
  },
  choose: ({ ref }) => toggle(ref.name),
  empty: () => 'no match',
});

function matching(query) {
  const remotes = $('opt-remote').checked, tags = $('opt-tag').checked;
  const selected = new Set(state.selected);
  const listed = ref => ref.group || selected.has(ref.name) || ref.kind === 'head' || (ref.kind === 'remote' && remotes) || (ref.kind === 'tag' && tags);
  const byDefault = (a, b) => (b.ref.group || 0) - (a.ref.group || 0) || selected.has(b.ref.name) - selected.has(a.ref.name) || a.order - b.order;
  return [...GROUPS.map(g => ({ ...g, group: true })), ...state.refs]
    .filter(listed)
    .map((ref, order) => ({ ref, order, match: fuzzy(query, ref.label || ref.name) }))
    .filter(x => x.match)
    .sort(query ? (a, b) => a.match.score - b.match.score || a.order - b.order : byDefault)
    .slice(0, MAX_SHOWN);
}

export function open() {
  if (state.repo.path) menu.open();
}

function toggle(name) {
  setSelection(state.selected.includes(name) ? state.selected.filter(n => n !== name) : [...state.selected, name]);
}

export function setSelection(names) {
  state.selected = names;
  saveSelection();
  return loadLog();
}

export function saveSelection() {
  state.store.set('sel', state.selected);
  const url = new URL(location);
  url.searchParams.delete('ref');
  url.searchParams.set('repo', state.repo.path);
  state.selected.forEach(name => url.searchParams.append('ref', name));
  window.history.replaceState(null, '', url);
  refreshChips();
}

function refreshChips() {
  const find = name => GROUPS.find(g => g.name === name) || state.refs.find(r => r.name === name) || { kind: 'head' };
  const chip = name => `<span class="chip k-${find(name).kind}">${esc(find(name).label || name)}</span>`;
  const more = state.selected.length > MAX_CHIPS ? `<span class="chip">+${state.selected.length - MAX_CHIPS}</span>` : '';
  $('chips').innerHTML = state.selected.length ? state.selected.slice(0, MAX_CHIPS).map(chip).join('') + more : '<span class="chip">HEAD</span>';
  if (menu.isOpen) menu.render();
}

onRefs(() => {
  const known = new Set([...state.refs.map(r => r.name), ...GROUPS.map(g => g.name)]);
  state.selected = state.selected.filter(name => known.has(name));
  refreshChips();
});

$('pick-btn').addEventListener('click', () => (menu.isOpen ? menu.close() : open()));

$('menu').addEventListener('click', e => {
  const act = e.target.dataset?.act;
  if (!act) return;
  setSelection({ current: state.repo.branch ? [state.repo.branch] : [], all: DEFAULT_SELECTION, none: [] }[act]);
  $('filter').focus();
});

['remote', 'tag'].forEach(kind => $(`opt-${kind}`).addEventListener('change', e => {
  state.store.set(kind, e.target.checked);
  menu.render();
  $('filter').focus();
}));
