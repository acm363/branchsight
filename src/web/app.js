import * as actions from './actions.js';
import { getJson, inDesktopApp, setRepo } from './api.js';
import * as branches from './branches.js';
import { $, closeLayer, copy, topLayer } from './dom.js';
import { color, esc, samePath } from './format.js';
import { DEFAULT_SELECTION } from './graph.js';
import * as history from './history.js';
import * as log from './log.js';
import * as panel from './panel.js';
import { prefs, repoPrefs } from './prefs.js';
import * as projects from './projects.js';
import * as settings from './settings.js';
import { commitAt, state } from './state.js';

const MAX_RECENT = 30;

function select(i, { open = false, scroll = 'nearest' } = {}) {
  if (!state.entries.length) return;
  log.setCursor(i, { scroll });
  if (open || panel.isOpen()) panel.showSoon(state.entries[state.cursor], open ? 0 : 120);
}

const page = () => Math.max(1, Math.floor($('list').clientHeight / log.rowHeight()) - 1);

const shortcuts = {
  ArrowDown: () => select(state.cursor + 1), j: () => select(state.cursor + 1),
  ArrowUp: () => select(state.cursor - 1), k: () => select(state.cursor - 1),
  PageDown: () => select(state.cursor + page()), PageUp: () => select(state.cursor - page()),
  Home: () => select(0), End: () => select(state.entries.length - 1),
  Enter: () => select(Math.max(0, state.cursor), { open: true }),
  b: branches.open, '/': branches.open, p: projects.open,
  r: () => history.reload(), f: actions.fetchRemotes,
  c: () => commitAt(state.cursor) && copy(commitAt(state.cursor).h),
};

document.addEventListener('keydown', e => {
  const layer = topLayer();
  if (layer?.keydown?.(e)) return;
  if (e.key === 'Escape') {
    e.preventDefault();
    if (layer) closeLayer(layer);
    else panel.close();
    return;
  }
  if (e.target.closest?.('input, select') || e.ctrlKey || e.metaKey || e.altKey) return;
  const action = shortcuts[e.key];
  if (!action) return;
  action();
  e.preventDefault();
});

$('rows').addEventListener('click', e => {
  const row = e.target.closest('.row');
  if (!row || e.detail > 1) return;
  const i = +row.dataset.i, commit = commitAt(i);
  if ((e.ctrlKey || e.metaKey) && state.cursor >= 0 && state.cursor !== i) return panel.showCompare(state.cursor, i);
  if (commit && e.target.closest('.h')) {
    copy(commit.h);
    select(i, { scroll: false });
  } else select(i, { scroll: false, open: true });
});

$('rows').addEventListener('dblclick', e => {
  if (e.target.closest('.row') && !e.target.closest('.h')) panel.close();
});

$('rows').addEventListener('contextmenu', actions.openContextMenu);

$('limit').addEventListener('change', e => {
  state.store.set('limit', e.target.value);
  history.loadLog();
});

$('reload').addEventListener('click', () => history.reload());

async function resolveRepo(fallback) {
  const params = new URLSearchParams(location.search);
  if (params.has('start')) return null;
  if (params.get('repo')) return params.get('repo');
  const host = location.hostname.endsWith('.localhost') && !inDesktopApp ? location.hostname.slice(0, -'.localhost'.length) : '';
  const match = host && (await projects.list()).find(p => p.name.toLowerCase() === host);
  return match ? match.path : fallback || prefs.get('last');
}

function remember(repo) {
  prefs.set('last', repo.path);
  prefs.set('recent', [repo.path, ...(prefs.get('recent') || []).filter(p => !samePath(p, repo.path))].slice(0, MAX_RECENT));
  document.title = `${repo.name} · BranchSight`;
  $('repo').textContent = repo.name;
  $('proj-chip').title = `${repo.path} — switch project (p)`;
  $('avatar').textContent = (repo.name[0] || '?').toUpperCase();
  $('avatar').style.background = color([...repo.name].reduce((h, ch) => (h * 31 + ch.charCodeAt(0)) >>> 0, 0));
}

function restoreView(store) {
  const fromUrl = new URLSearchParams(location.search).getAll('ref');
  state.selected = fromUrl.length ? fromUrl : store.get('sel') ?? DEFAULT_SELECTION;
  $('opt-remote').checked = store.get('remote') ?? true;
  $('opt-tag').checked = store.get('tag') ?? false;
  $('limit').value = store.get('limit') ?? '5000';
  $('auto-fetch').checked = store.get('autoFetch') ?? false;
  const width = store.get('detailWidth');
  if (width) $('detail').style.width = `${width}px`;
}

async function init() {
  settings.restore();
  const context = await getJson('/api/context');
  state.roots = context.roots;
  $('about').textContent = `BranchSight ${context.version} · MIT · github.com/acm363/branchsight`;
  const path = await resolveRepo(context.default);
  if (!path) return projects.showStart();
  setRepo(path);
  try {
    state.repo = await getJson('/api/repo');
  } catch (e) {
    if (samePath(path, prefs.get('last'))) prefs.set('last', null);
    setRepo(null);
    state.repo = {};
    return projects.showStart(e.message);
  }
  setRepo(state.repo.path);
  state.store = repoPrefs(state.repo.path);
  remember(state.repo);
  restoreView(state.store);
  await history.loadRefs();
  await branches.setSelection(state.selected);
  $('list').focus();
  history.startPolling();
  actions.startAutoFetch();
}

init().catch(e => {
  $('rows').innerHTML = `<div id="error">${esc(e.message)}</div>`;
});
