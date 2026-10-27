import { fetchRemotes, startAutoFetch } from './actions/commands.js';
import { openContextMenu } from './actions/context-menu.js';
import { getJson, inDesktopApp, setRepo } from './shared/api.js';
import * as branches from './header/branches.js';
import { $, closeLayer, copy, topLayer } from './shared/dom.js';
import { esc, samePath } from './shared/format.js';
import * as log from './history/list.js';
import * as panel from './changes/panel.js';
import { prefs, repoPrefs } from './shared/prefs.js';
import * as projects from './header/projects.js';
import * as settings from './header/settings.js';
import { commitAt, state } from './shared/state.js';
import * as sync from './history/sync.js';

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
  r: () => sync.reload({ fresh: true }), f: fetchRemotes,
  c: () => commitAt(state.cursor) && copy(commitAt(state.cursor).h),
};

const typing = target => target.closest?.('input, select');
const pressingFocusedButton = e => e.key === 'Enter' && e.target.closest?.('button');

document.addEventListener('keydown', e => {
  const layer = topLayer();
  if (layer?.keydown?.(e)) return;
  if (e.key === 'Escape') {
    e.preventDefault();
    if (layer) closeLayer(layer);
    else panel.close();
    return;
  }
  if (typing(e.target) || pressingFocusedButton(e) || e.ctrlKey || e.metaKey || e.altKey) return;
  const action = shortcuts[e.key];
  if (!action) return;
  action();
  e.preventDefault();
});

document.addEventListener('mousedown', e => {
  if (!e.target.closest('button') || e.target.closest('dialog')) return;
  e.preventDefault();
  if (document.activeElement?.matches('select')) document.activeElement.blur();
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

$('rows').addEventListener('contextmenu', openContextMenu);

$('limit').addEventListener('change', e => {
  state.store.set('limit', e.target.value);
  sync.loadLog();
});

$('reload').addEventListener('click', () => sync.reload({ fresh: true }));

async function resolveRepo(fallback) {
  const params = new URLSearchParams(location.search);
  if (params.has('start')) return null;
  if (params.get('repo')) return params.get('repo');
  const host = location.hostname.endsWith('.localhost') && !inDesktopApp ? location.hostname.slice(0, -'.localhost'.length) : '';
  const match = host && (await projects.list()).find(p => p.name.toLowerCase() === host);
  return match ? match.path : fallback || prefs.get('last');
}

function restoreView(store) {
  const fromUrl = new URLSearchParams(location.search).getAll('ref');
  state.selected = fromUrl.length ? fromUrl : store.get('sel') ?? branches.DEFAULT_SELECTION;
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
  projects.setCurrent(state.repo);
  restoreView(state.store);
  await sync.reload({ keepScroll: false }).catch(e => {
    $('rows').innerHTML = `<div id="error">${esc(e.message)} — retrying…</div>`;
  });
  branches.saveSelection();
  $('list').focus();
  sync.startPolling();
  startAutoFetch();
}

init().catch(e => {
  $('rows').innerHTML = `<div id="error">${esc(e.message)}</div>`;
});
