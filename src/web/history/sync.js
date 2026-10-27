import { getJson } from '../shared/api.js';
import { $ } from '../shared/dom.js';
import { esc, plural } from '../shared/format.js';
import { layout } from './graph.js';
import { indexRefs } from './labels.js';
import * as log from './list.js';
import { isWorktree, state } from '../shared/state.js';

const POLL_MS = 5000, LOG_CACHE_SIZE = 16;
const logCache = new Map();
const refListeners = [], logListeners = [];

let seen = null, shown = null, failed = false, polling = false, request = null;

export const onRefs = listener => refListeners.push(listener);

export const onLog = listener => logListeners.push(listener);

const cursorKey = () => log.entryKey(state.entries[state.cursor]) ?? null;

async function loadRefs() {
  state.refs = await getJson('/api/refs');
  state.refsByHash = indexRefs(state.refs);
  refListeners.forEach(listener => listener());
}

async function fetchLog(params, fresh, signal) {
  const key = history => `${state.repo.path}|${history}|${params}`;
  if (!fresh && logCache.has(key(seen?.history))) return logCache.get(key(seen.history));
  const { history, commits } = await getJson('/api/log', params, signal);
  const loaded = { commits, ...layout(commits), withRow: null };
  logCache.set(key(history), loaded);
  if (logCache.size > LOG_CACHE_SIZE) logCache.delete(logCache.keys().next().value);
  return loaded;
}

function withWorktree(loaded, status) {
  const dirty = status && status.staged + status.unstaged + status.untracked > 0;
  const { head } = state.repo;
  if (!dirty || (head && !loaded.commits.some(c => c.h === head))) return { entries: loaded.commits, rows: loaded.rows, lanes: loaded.lanes };
  const entries = [{ kind: 'worktree', h: '', p: head ? [head] : [], status }, ...loaded.commits];
  if (loaded.withRow?.head !== head) loaded.withRow = { head, ...layout(entries) };
  return { entries, rows: loaded.withRow.rows, lanes: loaded.withRow.lanes };
}

export async function loadLog({ keepScroll = false, fresh = false } = {}) {
  request?.abort();
  const mine = request = new AbortController();
  const params = new URLSearchParams([...state.selected.map(name => ['ref', name]), ['limit', $('limit').value]]);
  const pending = setTimeout(() => { if (!keepScroll) $('count').textContent = 'loading…'; }, 120);
  try {
    const loaded = await fetchLog(params, fresh, mine.signal);
    if (mine !== request) return;
    shown = loaded;
    failed = false;
    log.show(withWorktree(loaded, seen?.status), { cursorKey: cursorKey(), top: keepScroll ? log.anchor() : null });
    const limit = +$('limit').value, count = loaded.commits.length;
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
  logListeners.forEach(listener => listener(seen?.worktree));
}

export async function reload({ keepScroll = true, fresh = false, now = null } = {}) {
  try {
    const [current, repo] = await Promise.all([now ?? getJson('/api/state'), getJson('/api/repo')]);
    state.repo = repo;
    await loadRefs();
    seen = current;
  } catch (e) {
    failed = true;
    throw e;
  }
  await loadLog({ keepScroll, fresh });
}

function refreshWorktree(status) {
  if (!shown) return;
  const next = withWorktree(shown, status);
  if (isWorktree(state.entries[0]) && isWorktree(next.entries[0])) {
    state.entries[0].status = status;
    log.render();
  } else {
    log.show(next, { cursorKey: cursorKey(), top: log.anchor() });
  }
  logListeners.forEach(listener => listener(seen?.worktree));
}

export async function poll() {
  if (!state.repo.path || document.hidden || polling) return;
  polling = true;
  try {
    const now = await getJson('/api/state');
    if (failed || !seen || now.history !== seen.history) await reload({ now });
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
  setInterval(poll, POLL_MS);
  document.addEventListener('visibilitychange', poll);
}
