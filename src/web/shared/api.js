const tauri = window.__TAURI__?.core;
let repoPath = null;

export const inDesktopApp = Boolean(tauri);

export function setRepo(path) {
  repoPath = path;
}

function url(path, params = {}) {
  const query = new URLSearchParams(params);
  if (repoPath) query.set('repo', repoPath);
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
