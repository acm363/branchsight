import { getJson } from './api.js';
import { $ } from './dom.js';
import { esc, plural } from './format.js';
import { GROUPS, layout } from './graph.js';
import { indexRefs } from './labels.js';
import * as log from './log.js';
import * as panel from './panel.js';
import { isWorktree, state } from './state.js';

const POLL_MS = 5000, LOG_CACHE_SIZE = 16;
const logCache = new Map();
const listeners = [];

let seen = null, failed = false, polling = false, request = null;

export const onRefs = listener => listeners.push(listener);

export async function loadRefs() {
  state.refs = await getJson('/api/refs');
  state.refsByHash = indexRefs(state.refs);
  const known = new Set([...state.refs.map(r => r.name), ...GROUPS.map(g => g.name)]);
  state.selected = state.selected.filter(name => known.has(name));
  listeners.forEach(listener => listener());
}

async function fetchLog(params, history, signal) {
  const key = `${state.repo.path}|${history}|${params}`;
  if (logCache.has(key)) return logCache.get(key);
  const commits = await getJson('/api/log', params, signal);
  const entry = { commits, ...layout(commits) };
  logCache.set(key, entry);
  if (logCache.size > LOG_CACHE_SIZE) logCache.delete(logCache.keys().next().value);
  return entry;
}

const cursorKey = () => log.entryKey(state.entries[state.cursor]) ?? null;

function withWorktree({ commits, rows, lanes }, status) {
  const dirty = status && status.staged + status.unstaged + status.untracked > 0;
  const { head } = state.repo;
  if (!dirty || !head || !commits.some(c => c.h === head)) return { entries: commits, rows, lanes };
  const entries = [{ kind: 'worktree', h: '', p: [head], status }, ...commits];
  return { entries, ...layout(entries) };
}

export async function loadLog({ keepScroll = false } = {}) {
  request?.abort();
  const mine = request = new AbortController();
  const params = new URLSearchParams([...state.selected.map(name => ['ref', name]), ['limit', $('limit').value]]);
  const pending = setTimeout(() => { if (!keepScroll) $('count').textContent = 'loading…'; }, 120);
  try {
    const fresh = await getJson('/api/state', {}, mine.signal);
    const history = await fetchLog(params, fresh.history, mine.signal);
    if (mine !== request) return;
    log.show(withWorktree(history, fresh.status), { cursorKey: cursorKey(), top: keepScroll ? log.anchor() : null });
    seen = fresh;
    failed = false;
    const limit = +$('limit').value, count = history.commits.length;
    $('count').textContent = `${plural(count, 'commit')}${count >= limit ? ' (limit)' : ''}`;
  } catch (e) {
    if (e.name === 'AbortError' || mine !== request) return;
    failed = true;
    log.show({ entries: [], rows: [], lanes: 1 });
    $('count').textContent = '';
    $('rows').innerHTML = `<div id="error">${esc(e.message)} — retrying…</div>`;
    return;
  } finally {
    clearTimeout(pending);
  }
  syncWorktreePanel();
}

function syncWorktreePanel() {
  if (!panel.showingWorktree()) return;
  if (isWorktree(state.entries[0])) panel.showWorktree({ keep: true });
  else panel.close();
}

export async function reload({ keepScroll = true } = {}) {
  state.repo = await getJson('/api/repo');
  await loadRefs();
  await loadLog({ keepScroll });
}

function refreshWorktree(status) {
  const commits = state.entries.filter(e => !isWorktree(e));
  log.show(withWorktree({ commits, ...layout(commits) }, status), { cursorKey: cursorKey(), top: log.anchor() });
  syncWorktreePanel();
}

export async function poll() {
  if (!state.repo.path || document.hidden || polling) return;
  polling = true;
  try {
    const now = await getJson('/api/state');
    if (failed || !seen || now.history !== seen.history) await reload();
    else if (now.worktree !== seen.worktree) {
      seen = now;
      refreshWorktree(now.status);
    }
  } catch {
  } finally {
    polling = false;
  }
}

export function startPolling() {
  poll();
  setInterval(poll, POLL_MS);
  document.addEventListener('visibilitychange', poll);
}
