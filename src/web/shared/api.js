import { getLocale } from './i18n.js';

const tauri = window.__TAURI__?.core;
let repoPath = null;

export const inDesktopApp = Boolean(tauri);

export function setRepo(path) {
  repoPath = path;
}

function url(path, params = {}) {
  const query = new URLSearchParams(params);
  if (repoPath) query.set('repo', repoPath);
  query.set('lang', getLocale());
  return `${path}?${query}`;
}

async function call(method, path, signal) {
  if (tauri) {
    const [status, body] = await tauri.invoke('api', { method, path }).catch(e => {
      throw e instanceof Error ? e : new Error(String(e));
    });
    if (signal?.aborted) throw new DOMException('superseded', 'AbortError');
    if (status !== 200) throw new Error(body);
    return body;
  }
  const options = { method, signal, headers: { 'X-BranchSight': '1' } };
  let reply;
  try {
    reply = await fetch(path, options);
  } catch (e) {
    if (e.name === 'AbortError' || method !== 'GET') throw e;
    await new Promise(resolve => setTimeout(resolve, 400));
    reply = await fetch(path, options);
  }
  const body = await reply.text();
  if (!reply.ok) throw new Error(body);
  return body;
}

export const getJson = (path, params, signal) => call('GET', url(path, params), signal).then(JSON.parse);
export const post = (path, params) => call('POST', url(path, params)).then(JSON.parse);
export const pickFolder = () => tauri?.invoke('pick_folder') ?? Promise.resolve(null);

export const newChannel = () => (tauri ? new tauri.Channel() : null);
export const termShells = () => tauri?.invoke('term_shells') ?? Promise.resolve([]);
export const termOpen = (id, shell, cols, rows, cwd, onData) => tauri?.invoke('term_open', { id, shell, cols, rows, cwd, onData });
export const termWrite = (id, data) => tauri?.invoke('term_write', { id, data });
export const termResize = (id, cols, rows) => tauri?.invoke('term_resize', { id, cols, rows });
export const termClose = id => tauri?.invoke('term_close', { id });
export const termExternal = cwd => tauri?.invoke('term_external', { cwd });
