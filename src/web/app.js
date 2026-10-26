import { DEFAULT_SELECTION, GROUPS, layout } from './graph.js';

const MAX_DIFF_LINES = 40000, FONT_SIZES = { min: 10, max: 20, default: 13 };
let ROW = 21, LANE = 17;
const POLL_MS = 5000, AUTO_FETCH_MS = 120000;
const COLORS = ['#3b82f6', '#e5484d', '#30a46c', '#f5a524', '#8e4ec6', '#12a594', '#d6409f', '#0090ff', '#ad7f58', '#7c66dc', '#e16f24', '#5bb98b'];
const $ = id => document.getElementById(id);
const esc = s => s.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const color = n => COLORS[n % COLORS.length];

let repo = {}, refs = [], selected = [], commits = [], rows = [], lanes = 1, cursor = -1, openHash = null;
let refsByHash = new Map(), menuItems = [], menuCursor = 0, detailTimer = 0;
let logRequest = null;
const logCache = new Map(), LOG_CACHE_SIZE = 16;
let ctx = { roots: [] }, repoPath = null, projectList = [], pItems = [], pCursor = 0;

const store = {
  get(k) { try { return JSON.parse(localStorage.getItem(`branchsight:${repo.path}:${k}`)); } catch { return null; } },
  set(k, v) { try { localStorage.setItem(`branchsight:${repo.path}:${k}`, JSON.stringify(v)); } catch {} },
};

const prefs = {
  get(k) { try { return JSON.parse(localStorage.getItem(`branchsight:${k}`)); } catch { return null; } },
  set(k, v) { try { localStorage.setItem(`branchsight:${k}`, JSON.stringify(v)); } catch {} },
};

function withRepo(path, params = new URLSearchParams()) {
  if (repoPath) params.set('repo', repoPath);
  return `${path}?${params}`;
}

const tauri = window.__TAURI__?.core;

async function api(path, signal, method = 'GET') {
  if (tauri) {
    const [status, body] = await tauri.invoke('api', { method, path });
    if (signal?.aborted) throw new DOMException('superseded', 'AbortError');
    if (status !== 200) throw new Error(body);
    return { json: async () => JSON.parse(body), text: async () => body };
  }
  const r = await fetch(path, { signal, method });
  if (!r.ok) throw new Error(await r.text());
  return r;
}

const THEMES = ['system', 'light', 'dark'];

function setTheme(theme) {
  const choice = THEMES.includes(theme) ? theme : 'system';
  if (choice === 'system') document.documentElement.removeAttribute('data-theme');
  else document.documentElement.dataset.theme = choice;
  $('theme-seg').querySelectorAll('button').forEach(b => b.classList.toggle('on', b.dataset.theme === choice));
  prefs.set('theme', choice);
  if (commits.length) render();
}

function setFontSize(px) {
  const size = Math.max(FONT_SIZES.min, Math.min(FONT_SIZES.max, px));
  ROW = Math.round(size * 1.6);
  LANE = Math.round(ROW * 0.8);
  const root = document.documentElement.style;
  root.setProperty('--fs', `${size}px`);
  root.setProperty('--row', `${ROW}px`);
  $('fs-val').textContent = `${size}px`;
  prefs.set('fontSize', size);
  if (commits.length) {
    $('spacer').style.height = `${commits.length * ROW}px`;
    applyCols();
    render();
  }
}

function fmtDate(t) {
  const d = new Date(t * 1000), p = n => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

const AUTO_LANES = 10;
const COLS = { graph: [30, 640], author: [40, 400], date: [60, 220], hash: [40, 200] };
let colWidths = {};
const graphWidth = () => colWidths.graph || Math.round(LANE * 1.1) + Math.min(lanes, AUTO_LANES) * LANE;

function applyCols() {
  const pane = $('pane');
  ['author', 'date', 'hash'].forEach(k => colWidths[k]
    ? pane.style.setProperty(`--w-${k}`, `${colWidths[k]}px`)
    : pane.style.removeProperty(`--w-${k}`));
  pane.style.setProperty('--w-graph', `${graphWidth()}px`);
}

const ICONS = {
  branch: '<svg viewBox="0 0 16 16"><path d="M5 3.5a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0Zm0 9a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0Zm9-6a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0ZM3.5 5v6M12.5 8c0 2-2 3-4 3H3.5" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>',
  tag: '<svg viewBox="0 0 16 16"><path d="M2 2.5h5.6l6.4 6.4-5.1 5.1L2.5 7.6V2.5Z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/><circle cx="5.2" cy="5.7" r="1.1" fill="currentColor"/></svg>',
};

function refLabels(c, laneColor) {
  const list = refsByHash.get(c.h) || [];
  const heads = list.filter(r => r.kind === 'head');
  const remotes = list.filter(r => r.kind === 'remote');
  const merged = new Set();
  const label = (cls, icon, body, data) => `<span class="ref ${cls}" style="--c:${laneColor}"`
    + ` data-kind="${data.kind}" data-name="${esc(data.name)}" data-remotes="${esc((data.remotes || []).join(' '))}">${ICONS[icon]}${body}</span>`;
  const local = heads.map(h => {
    const tracking = remotes.filter(r => r.name.endsWith(`/${h.name}`));
    tracking.forEach(r => merged.add(r.name));
    const suffix = tracking.map(r => `<span class="remote">${esc(r.name.slice(0, -h.name.length - 1))}</span>`).join('');
    return label(`local${h.name === repo.branch ? ' current' : ''}`, 'branch', `<span class="name">${esc(h.name)}</span>${suffix}`,
      { kind: 'head', name: h.name, remotes: tracking.map(r => r.name) });
  });
  const remoteOnly = remotes.filter(r => !merged.has(r.name))
    .map(r => label('remote-only', 'branch', `<span class="name">${esc(r.name)}</span>`, { kind: 'remote', name: r.name, remotes: [r.name] }));
  const tags = list.filter(r => r.kind === 'tag').map(r => label('tag', 'tag', `<span class="name">${esc(r.name)}</span>`, { kind: 'tag', name: r.name }));
  const detached = c.h === repo.head && !repo.branch ? [label('local current', 'branch', '<span class="name">HEAD</span>', { kind: 'detached', name: 'HEAD' })] : [];
  return [...detached, ...local, ...remoteOnly, ...tags].join('');
}

function render() {
  const list = $('list'), top = list.scrollTop, h = list.clientHeight;
  const first = Math.max(0, Math.floor(top / ROW) - 4);
  const last = Math.min(commits.length, Math.ceil((top + h) / ROW) + 4);
  const gw = graphWidth();
  let html = '';
  for (let i = first; i < last; i++) {
    const c = commits[i];
    html += `<div class="row${i === cursor ? ' sel' : ''}${c.p.length > 1 ? ' merge' : ''}" data-i="${i}" style="top:${i * ROW}px">`
      + `<span class="s">${refLabels(c, color(rows[i].color))}<span class="t">${esc(c.s)}</span></span>`
      + `<span class="a">${esc(c.a)}</span><span class="d">${fmtDate(c.t)}</span>`
      + `<span class="h" title="Copy ${c.h}">${c.h.slice(0, 7)}</span></div>`;
  }
  $('rows').innerHTML = html;
  draw(first, last, gw);
}

function draw(first, last, gw) {
  const cv = $('graph'), dpr = window.devicePixelRatio || 1, hpx = Math.max(1, (last - first) * ROW);
  cv.style.top = `${first * ROW}px`;
  cv.style.width = `${gw}px`;
  cv.style.height = `${hpx}px`;
  cv.width = gw * dpr;
  cv.height = hpx * dpr;
  const g = cv.getContext('2d');
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.lineWidth = ROW * 0.12;
  g.lineCap = 'round';
  const X = j => Math.round(LANE * 0.55) + j * LANE;
  const seg = (x1, y1, x2, y2, stroke, dashed) => {
    g.strokeStyle = stroke;
    g.setLineDash(dashed ? [2, 3] : []);
    g.beginPath();
    g.moveTo(x1, y1);
    if (x1 === x2) g.lineTo(x2, y2);
    else { const m = (y1 + y2) / 2; g.bezierCurveTo(x1, m, x2, m, x2, y2); }
    g.stroke();
  };
  const bg = getComputedStyle(document.body).backgroundColor;
  for (let i = first; i < last; i++) {
    const r = rows[i], c = commits[i];
    const y0 = (i - first) * ROW, mid = y0 + ROW / 2, y1 = y0 + ROW;
    r.before.forEach((h, j) => {
      if (h == null) return;
      if (h === c.h) seg(X(j), y0, X(r.col), mid, color(r.bc[j]));
      else seg(X(j), y0, X(j), y1, color(r.bc[j]));
    });
    r.out.forEach(([t, n, known]) => seg(X(r.col), mid, X(t), y1, color(n), !known));
    g.setLineDash([]);
    g.beginPath();
    g.arc(X(r.col), mid, ROW * (c.p.length > 1 ? 0.2 : 0.24), 0, Math.PI * 2);
    g.fillStyle = c.p.length > 1 ? bg : color(r.color);
    g.strokeStyle = color(r.color);
    g.fill();
    g.stroke();
    if (c.h === repo.head) {
      g.beginPath();
      g.arc(X(r.col), mid, ROW * 0.39, 0, Math.PI * 2);
      g.stroke();
    }
  }
}

function setCursor(i, { scroll = true, open = false } = {}) {
  if (!commits.length) return;
  cursor = Math.max(0, Math.min(commits.length - 1, i));
  if (scroll) {
    const list = $('list'), y = cursor * ROW;
    if (y < list.scrollTop) list.scrollTop = y;
    else if (y + ROW > list.scrollTop + list.clientHeight) list.scrollTop = y + ROW - list.clientHeight;
  }
  render();
  if (open || !$('detail').hidden) {
    clearTimeout(detailTimer);
    detailTimer = setTimeout(() => showDetail(commits[cursor]), open ? 0 : 120);
  }
}

async function showDetail(c) {
  if (!c) return;
  openHash = c.h;
  $('detail').hidden = $('split').hidden = false;
  $('detail-title').innerHTML = `<span class="h" title="Copy ${c.h}">${c.h.slice(0, 10)}</span>${esc(c.s)}`;
  $('detail-title').title = c.s;
  const text = await (await api(withRepo('/api/commit', new URLSearchParams({ h: c.h })))).text();
  if (openHash !== c.h) return;
  const lines = text.split('\n');
  const files = [];
  let head = '', file = null;
  lines.slice(0, MAX_DIFF_LINES).forEach(line => {
    let cls = '';
    if (line.startsWith('diff --git ')) {
      file = { path: diffPath(line), st: 'M', add: 0, del: 0, id: `f${files.length}`, body: '' };
      files.push(file);
      return;
    }
    if (file) {
      if (/^(new file mode)/.test(line)) file.st = 'A';
      else if (/^(deleted file mode)/.test(line)) file.st = 'D';
      else if (/^rename from /.test(line)) { file.st = 'R'; file.from = line.slice(12); }
      if (line.startsWith('@@')) cls = 'hk';
      else if (line.startsWith('+++') || line.startsWith('---') || /^(index |new file|deleted file|similarity|dissimilarity|rename |copy |old mode|new mode|Binary files)/.test(line)) cls = 'mt';
      else if (line[0] === '+') { cls = 'ad'; file.add++; }
      else if (line[0] === '-') { cls = 'rm'; file.del++; }
    }
    const span = `<span${cls ? ` class="${cls}"` : ''}>${esc(line) || ' '}</span>`;
    if (file) file.body += span;
    else head += span;
  });
  const label = f => esc(f.from ? `${f.from} → ${f.path}` : f.path);
  const sections = files.map(f => `<div class="fsec" id="${f.id}"><div class="fh"><span class="st st-${f.st}">${f.st}</span>`
    + `<span class="p">${label(f)}</span><span class="n"><b>+${f.add}</b> <i>−${f.del}</i></span>`
    + `<button class="fold" title="Fold or unfold this file">▾</button></div><div class="fbody">${f.body}</div></div>`).join('');
  const more = lines.length > MAX_DIFF_LINES ? `<span class="mt">… ${lines.length - MAX_DIFF_LINES} more lines</span>` : '';
  $('diff').innerHTML = head + sections + more;
  $('diff').scrollTop = 0;
  $('files-count').textContent = `${files.length} file${files.length === 1 ? '' : 's'}`;
  updateFoldAll();
  $('files').innerHTML = files.map(f => `<div class="file" data-id="${f.id}" title="${esc(f.from ? `${f.from} → ${f.path}` : f.path)}">`
    + `<span class="st st-${f.st}">${f.st}</span><span class="p">${label(f)}</span>`
    + `<span class="n"><b>+${f.add}</b> <i>−${f.del}</i></span></div>`).join('') || '<div class="file"><span class="p">no file changes</span></div>';
  render();
}

function updateFoldAll() {
  const sections = [...$('diff').querySelectorAll('.fsec')];
  $('fold-all').textContent = sections.some(s => !s.classList.contains('closed')) ? 'Collapse all' : 'Expand all';
  $('fold-all').disabled = !sections.length;
}

function diffPath(line) {
  const s = line.slice(11);
  const half = (s.length - 1) / 2;
  if (Number.isInteger(half) && s.slice(2, half) === s.slice(half + 3)) return s.slice(half + 3);
  const i = s.lastIndexOf(' b/');
  return i >= 0 ? s.slice(i + 3) : s;
}

function closeDetail() {
  clearTimeout(detailTimer);
  openHash = null;
  $('detail').hidden = $('split').hidden = true;
  requestAnimationFrame(render);
}

function renderChips() {
  const shown = selected.slice(0, 5);
  const find = n => GROUPS.find(g => g.name === n) || refs.find(r => r.name === n) || { kind: 'head' };
  $('chips').innerHTML = selected.length
    ? shown.map(n => `<span class="chip k-${find(n).kind}">${esc(find(n).label || n)}</span>`).join('') + (selected.length > 5 ? `<span class="chip">+${selected.length - 5}</span>` : '')
    : '<span class="chip">HEAD</span>';
}

function fuzzy(q, s) {
  if (!q) return { score: 0, marks: [] };
  const ls = s.toLowerCase(), lq = q.toLowerCase();
  const sub = ls.indexOf(lq);
  if (sub >= 0) return { score: sub - 1000 + s.length / 100, marks: [...Array(lq.length).keys()].map(k => sub + k) };
  let score = 0, last = -1;
  const marks = [];
  for (const ch of lq) {
    const i = ls.indexOf(ch, last + 1);
    if (i < 0) return null;
    score += i - last - 1;
    marks.push(i);
    last = i;
  }
  return { score: score + s.length / 100, marks };
}

function highlight(s, marks) {
  const set = new Set(marks);
  return [...s].map((ch, i) => set.has(i) ? `<mark>${esc(ch)}</mark>` : esc(ch)).join('');
}

function renderMenu() {
  const q = $('filter').value.trim();
  const showRemote = $('opt-remote').checked, showTag = $('opt-tag').checked;
  const sel = new Set(selected);
  menuItems = [...GROUPS.map(g => ({ ...g, group: true })), ...refs]
    .filter(r => r.group || sel.has(r.name) || r.kind === 'head' || (r.kind === 'remote' && showRemote) || (r.kind === 'tag' && showTag))
    .map((r, order) => ({ r, order, m: fuzzy(q, r.label || r.name) }))
    .filter(x => x.m)
    .sort((a, b) => q ? a.m.score - b.m.score || a.order - b.order : (b.r.group || 0) - (a.r.group || 0) || (sel.has(b.r.name) - sel.has(a.r.name)) || a.order - b.order)
    .slice(0, 400);
  menuCursor = Math.min(menuCursor, Math.max(0, menuItems.length - 1));
  $('reflist').innerHTML = menuItems.map(({ r, m }, i) =>
    `<div class="ref k-${r.kind}${r.group ? ' group' : ''}${sel.has(r.name) ? ' on' : ''}${i === menuCursor ? ' cur' : ''}" data-i="${i}">`
    + `<span class="box">${sel.has(r.name) ? '✓' : ''}</span><span class="name">${highlight(r.label || r.name, m.marks)}</span>`
    + `<span class="when">${r.date ? fmtDate(r.date).slice(0, 10) : ''}</span></div>`).join('') || '<div class="ref"><span class="name">no match</span></div>';
  $('reflist').querySelector('.cur')?.scrollIntoView({ block: 'nearest' });
}

function toggleRef(name) {
  selected = selected.includes(name) ? selected.filter(n => n !== name) : [...selected, name];
  selectionChanged();
}

function selectionChanged() {
  store.set('sel', selected);
  const url = new URL(location);
  url.searchParams.delete('ref');
  url.searchParams.set('repo', repo.path);
  selected.forEach(n => url.searchParams.append('ref', n));
  history.replaceState(null, '', url);
  renderChips();
  if (!$('menu').hidden) renderMenu();
  return loadLog();
}

function openMenu() {
  if (!repo.path) return;
  $('pmenu').hidden = true;
  $('menu').hidden = false;
  $('filter').value = '';
  menuCursor = 0;
  renderMenu();
  $('filter').focus();
}

function closeMenu() {
  $('menu').hidden = true;
  $('list').focus();
}

async function loadRefs() {
  refs = await (await api(withRepo('/api/refs'))).json();
  refsByHash = new Map();
  const order = { head: 0, remote: 1, tag: 2 };
  [...refs].sort((a, b) => order[a.kind] - order[b.kind]).forEach(r => {
    if (!refsByHash.has(r.hash)) refsByHash.set(r.hash, []);
    refsByHash.get(r.hash).push(r);
  });
  const names = new Set(refs.map(r => r.name));
  GROUPS.forEach(g => names.add(g.name));
  names.add('HEAD');
  selected = selected.filter(n => names.has(n));
  renderChips();
}

async function fetchLog(params, signal, fresh) {
  const key = `${repo.path}|${version}|${params}`;
  if (!fresh && logCache.has(key)) return logCache.get(key);
  const list = await (await api(withRepo('/api/log', params), signal)).json();
  const entry = { list, ...layout(list) };
  logCache.set(key, entry);
  if (logCache.size > LOG_CACHE_SIZE) logCache.delete(logCache.keys().next().value);
  return entry;
}

async function loadLog({ anchor = false, fresh = false } = {}) {
  const params = new URLSearchParams(selected.map(n => ['ref', n]));
  params.set('limit', $('limit').value);
  const keep = commits[cursor]?.h;
  const list = $('list'), topIndex = Math.floor(list.scrollTop / ROW);
  const top = anchor ? { h: commits[topIndex]?.h, offset: list.scrollTop - topIndex * ROW } : null;
  logRequest?.abort();
  const request = logRequest = new AbortController();
  const pending = setTimeout(() => { if (!anchor) $('count').textContent = 'loading…'; }, 120);
  try {
    const entry = await fetchLog(params, request.signal, fresh);
    if (request !== logRequest) return;
    ({ list: commits, rows, lanes } = entry);
  } catch (e) {
    if (e.name === 'AbortError') return;
    commits = [];
    loadFailed = true;
    $('count').textContent = '';
    $('rows').innerHTML = `<div id="error">${esc(e.message)} — retrying…</div>`;
    return;
  }
  finally {
    clearTimeout(pending);
  }
  loadFailed = false;
  applyCols();
  $('spacer').style.height = `${commits.length * ROW}px`;
  const limit = +$('limit').value;
  $('count').textContent = `${commits.length.toLocaleString()} commits${commits.length >= limit ? ' (limit)' : ''}`;
  const i = keep ? commits.findIndex(c => c.h === keep) : -1;
  cursor = i;
  const topAgain = top?.h ? commits.findIndex(c => c.h === top.h) : -1;
  if (topAgain >= 0) list.scrollTop = topAgain * ROW + top.offset;
  else if (i < 0) list.scrollTop = 0;
  render();
}

async function reload({ anchor = false, fresh = false } = {}) {
  repo = await (await api(withRepo('/api/repo'))).json();
  await loadRefs();
  await loadLog({ anchor, fresh });
}

let version = null, polling = false, fetching = false, lastFetch = 0, loadFailed = false;

async function poll() {
  if (!repo.path || document.hidden || polling) return;
  polling = true;
  try {
    const { v } = await (await api(withRepo('/api/state'))).json();
    const changed = version !== null && v !== version;
    version = v;
    if (loadFailed || changed) await reload({ anchor: true });
  } catch {
  } finally {
    polling = false;
  }
}

async function fetchRemotes() {
  if (fetching || !repo.path) return;
  fetching = true;
  const btn = $('fetch');
  btn.disabled = true;
  btn.textContent = '⇣ fetching…';
  try {
    await api(withRepo('/api/fetch'), undefined, 'POST');
    lastFetch = Date.now();
    btn.title = `Fetch all remotes (f) · last ${new Date().toLocaleTimeString()}`;
    btn.classList.remove('err');
  } catch (e) {
    lastFetch = Date.now();
    btn.title = `Fetch failed: ${e.message}`;
    btn.classList.add('err');
    showToast(`fetch failed: ${e.message.split('\n')[0]}`, 5000);
  } finally {
    fetching = false;
    btn.disabled = false;
    btn.textContent = '⇣ fetch';
    poll();
  }
}

function autoFetchTick() {
  if ($('auto-fetch').checked && !document.hidden && Date.now() - lastFetch >= AUTO_FETCH_MS) fetchRemotes();
}

async function loadProjects() {
  projectList = await (await api('/api/projects')).json();
}

function shortPath(path) {
  const root = ctx.roots.find(r => path.startsWith(`${r}/`));
  return root ? path.slice(root.length + 1) : path;
}

function renderProjects() {
  const q = $('pfilter').value.trim();
  const recent = prefs.get('recent') || [];
  const rank = p => { const i = recent.indexOf(p.path); return i < 0 ? recent.length : i; };
  pItems = projectList
    .map((p, order) => ({ p, order, m: fuzzy(q, p.name) || (q.includes('/') ? fuzzy(q, shortPath(p.path)) && { score: 500, marks: [] } : null) }))
    .filter(x => x.m)
    .sort((a, b) => q ? a.m.score - b.m.score || a.order - b.order : rank(a.p) - rank(b.p) || a.order - b.order);
  if (/^(\/|~|[a-zA-Z]:[\\/])/.test(q)) pItems.unshift({ p: { name: `open ${q}`, path: q }, m: { marks: [] } });
  pCursor = Math.min(pCursor, Math.max(0, pItems.length - 1));
  $('plist').innerHTML = pItems.map(({ p, m }, i) =>
    `<div class="ref${p.path === repo.path ? ' on' : ''}${i === pCursor ? ' cur' : ''}" data-i="${i}">`
    + `<span class="name">${highlight(p.name, m.marks)}</span><span class="when">${shortPath(p.path) === p.name ? '' : esc(shortPath(p.path))}</span></div>`).join('')
    || '<div class="ref"><span class="name">no git repository under the roots</span></div>';
  $('plist').querySelector('.cur')?.scrollIntoView({ block: 'nearest' });
}

async function openProjects() {
  closeMenu();
  $('pmenu').hidden = false;
  $('pfilter').value = '';
  pCursor = 0;
  $('pfilter').focus();
  await loadProjects();
  renderProjects();
}

function closeProjects() {
  if (!repo.path) return;
  $('pmenu').hidden = true;
  $('list').focus();
}

function switchProject(path) {
  if (path === repo.path) return closeProjects();
  location.href = `${location.pathname}?repo=${encodeURIComponent(path)}`;
}

function closeProject() {
  prefs.set('last', null);
  location.href = `${location.pathname}?start`;
}

function showStart() {
  document.body.classList.add('start');
  document.title = 'BranchSight';
  $('repo').textContent = 'no project';
  return openProjects();
}

async function resolveRepo() {
  const params = new URLSearchParams(location.search);
  if (params.has('start')) return null;
  if (params.get('repo')) return params.get('repo');
  const host = location.hostname.endsWith('.localhost') ? location.hostname.slice(0, -'.localhost'.length) : '';
  if (host) {
    await loadProjects();
    const match = projectList.find(p => p.name.toLowerCase() === host);
    if (match) return match.path;
  }
  return ctx.default || prefs.get('last');
}

async function init() {
  setTheme(prefs.get('theme'));
  setFontSize(prefs.get('fontSize') ?? FONT_SIZES.default);
  ctx = await (await api('/api/context')).json();
  $('about').textContent = `BranchSight ${ctx.version} · MIT · github.com/acm363/branchsight`;
  repoPath = await resolveRepo();
  if (repoPath) {
    try {
      repo = await (await api(withRepo('/api/repo'))).json();
    } catch (e) {
      $('rows').innerHTML = `<div id="error">${esc(e.message)}</div>`;
      if (repoPath === prefs.get('last')) prefs.set('last', null);
      repoPath = null;
    }
  }
  if (!repoPath) return showStart();
  repoPath = repo.path;
  prefs.set('last', repo.path);
  prefs.set('recent', [repo.path, ...(prefs.get('recent') || []).filter(p => p !== repo.path)].slice(0, 30));
  document.title = `${repo.name} · BranchSight`;
  $('repo').textContent = repo.name;
  $('proj-chip').title = `${repo.path} — switch project (p)`;
  $('avatar').textContent = (repo.name[0] || '?').toUpperCase();
  $('avatar').style.background = color([...repo.name].reduce((h, ch) => (h * 31 + ch.charCodeAt(0)) >>> 0, 0));
  const fromUrl = new URLSearchParams(location.search).getAll('ref');
  selected = fromUrl.length ? fromUrl : store.get('sel') ?? DEFAULT_SELECTION;
  $('opt-remote').checked = store.get('remote') ?? true;
  $('opt-tag').checked = store.get('tag') ?? false;
  $('limit').value = store.get('limit') ?? '5000';
  colWidths = prefs.get('cols') || {};
  const w = store.get('detailWidth');
  if (w) $('detail').style.width = `${w}px`;
  $('auto-fetch').checked = store.get('autoFetch') ?? false;
  await loadRefs();
  selectionChanged();
  $('list').focus();
  poll();
  setInterval(poll, POLL_MS);
  setInterval(autoFetchTick, 10000);
  autoFetchTick();
}

$('list').addEventListener('scroll', () => requestAnimationFrame(render), { passive: true });
window.addEventListener('resize', () => requestAnimationFrame(render));
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => commits.length && render());
let toastTimer = 0;
async function copyHash(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = Object.assign(document.createElement('textarea'), { value: text });
    document.body.append(ta);
    ta.select();
    document.execCommand('copy');
    ta.remove();
  }
  showToast(`copied ${text}`);
}

function confirmAction(text, okLabel) {
  const dialog = $('confirm');
  $('confirm-text').textContent = text;
  dialog.querySelector('[value="ok"]').textContent = okLabel;
  dialog.returnValue = '';
  dialog.showModal();
  return new Promise(resolve => dialog.addEventListener('close', () => resolve(dialog.returnValue === 'ok'), { once: true }));
}

async function gitAction(endpoint, name, busy) {
  showToast(busy, 60000);
  try {
    const { out } = await (await api(withRepo(endpoint, new URLSearchParams({ ref: name })), undefined, 'POST')).json();
    showToast(out, 3000);
  } catch (e) {
    showToast(e.message.split('\n')[0], 7000);
  }
  await reload({ anchor: true, fresh: true });
}

function focusCommit(hash) {
  const i = commits.findIndex(c => c.h === hash);
  if (i < 0) return;
  const list = $('list');
  cursor = i;
  list.scrollTop = Math.max(0, i * ROW - list.clientHeight / 2 + ROW / 2);
  render();
}

async function checkout(name, label) {
  await gitAction('/api/checkout', name, `checking out ${label}…`);
  if (repo.head && !commits.some(c => c.h === repo.head) && !selected.includes('HEAD')) {
    selected = [...selected, 'HEAD'];
    await selectionChanged();
  }
  focusCommit(repo.head);
}

async function deleteRemote(name) {
  const ok = await confirmAction(`Delete ${name} on the remote? Everyone who uses it loses the branch; BranchSight cannot undo it.`, 'Delete');
  if (ok) await gitAction('/api/delete-remote', name, `deleting ${name}…`);
}

function closeContext() {
  $('ctx').hidden = true;
}

function openContext(e) {
  const row = e.target.closest('.row');
  if (!row) return;
  e.preventDefault();
  const c = commits[+row.dataset.i];
  const ref = e.target.closest('.ref')?.dataset;
  const items = [];
  if (ref?.kind === 'head' || ref?.kind === 'remote') {
    items.push([`Checkout ${ref.name}`, () => checkout(ref.name, ref.name)]);
    items.push(['Copy branch name', () => copyHash(ref.kind === 'remote' ? ref.name.slice(ref.name.indexOf('/') + 1) : ref.name)]);
    ref.remotes.split(' ').filter(Boolean).forEach(r => items.push([`Delete remote branch ${r}`, () => deleteRemote(r), 'danger']));
  }
  if (ref?.kind === 'tag') {
    items.push([`Checkout tag ${ref.name} (detached)`, () => checkout(ref.name, ref.name)]);
    items.push(['Copy tag name', () => copyHash(ref.name)]);
  }
  if (items.length) items.push(null);
  items.push([`Checkout commit ${c.h.slice(0, 7)} (detached)`, () => checkout(c.h, c.h.slice(0, 7))]);
  items.push(['Copy commit hash', () => copyHash(c.h)]);
  const menu = $('ctx');
  menu.innerHTML = items.map((it, i) => it ? `<button data-i="${i}"${it[2] ? ` class="${it[2]}"` : ''}>${esc(it[0])}</button>` : '<hr>').join('');
  menu.onclick = ev => {
    const btn = ev.target.closest('button[data-i]');
    if (!btn) return;
    closeContext();
    items[+btn.dataset.i][1]();
  };
  menu.hidden = false;
  const { innerWidth: w, innerHeight: h } = window, r = menu.getBoundingClientRect();
  menu.style.left = `${Math.min(e.clientX, w - r.width - 4)}px`;
  menu.style.top = `${Math.min(e.clientY, h - r.height - 4)}px`;
}

function showToast(text, ms = 1400) {
  $('toast').textContent = text;
  $('toast').classList.add('on');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => $('toast').classList.remove('on'), ms);
}

$('rows').addEventListener('click', e => {
  const row = e.target.closest('.row');
  if (!row || e.detail > 1) return;
  if (e.target.closest('.h')) {
    copyHash(commits[+row.dataset.i].h);
    setCursor(+row.dataset.i, { scroll: false });
  } else setCursor(+row.dataset.i, { scroll: false, open: true });
});
$('rows').addEventListener('contextmenu', openContext);
$('rows').addEventListener('dblclick', e => {
  if (e.target.closest('.row') && !e.target.closest('.h')) closeDetail();
});
$('list').addEventListener('scroll', closeContext, { passive: true });
$('detail-title').addEventListener('click', e => { if (e.target.closest('.h') && openHash) copyHash(openHash); });
$('pick-btn').addEventListener('click', () => $('menu').hidden ? openMenu() : closeMenu());
$('filter').addEventListener('input', () => { menuCursor = 0; renderMenu(); });
$('filter').addEventListener('keydown', e => {
  if (e.key === 'ArrowDown') { menuCursor = Math.min(menuItems.length - 1, menuCursor + 1); renderMenu(); e.preventDefault(); }
  else if (e.key === 'ArrowUp') { menuCursor = Math.max(0, menuCursor - 1); renderMenu(); e.preventDefault(); }
  else if (e.key === 'Enter' || e.key === 'Tab') { const it = menuItems[menuCursor]; if (it) toggleRef(it.r.name); e.preventDefault(); }
  else if (e.key === 'Escape') { closeMenu(); e.preventDefault(); e.stopPropagation(); }
});
$('reflist').addEventListener('click', e => {
  const el = e.target.closest('.ref[data-i]');
  if (!el) return;
  menuCursor = +el.dataset.i;
  toggleRef(menuItems[menuCursor].r.name);
  $('filter').focus();
});
$('menu').addEventListener('click', e => {
  const act = e.target.dataset?.act;
  if (!act) return;
  if (act === 'current') selected = repo.branch ? [repo.branch] : [];
  if (act === 'all') selected = DEFAULT_SELECTION;
  if (act === 'none') selected = [];
  selectionChanged();
  $('filter').focus();
});
['remote', 'tag'].forEach(k => $(`opt-${k}`).addEventListener('change', e => { store.set(k, e.target.checked); renderMenu(); $('filter').focus(); }));
document.addEventListener('mousedown', e => {
  if (!$('ctx').hidden && !$('ctx').contains(e.target)) closeContext();
  if (!$('mainmenu').contains(e.target)) closeMenubar();
  if (!$('menu').hidden && !$('picker').contains(e.target)) closeMenu();
  if (!$('pmenu').hidden && !$('projects').contains(e.target)) closeProjects();
});
$('pfilter').addEventListener('input', () => { pCursor = 0; renderProjects(); });
$('pfilter').addEventListener('keydown', e => {
  if (e.key === 'ArrowDown') { pCursor = Math.min(pItems.length - 1, pCursor + 1); renderProjects(); e.preventDefault(); }
  else if (e.key === 'ArrowUp') { pCursor = Math.max(0, pCursor - 1); renderProjects(); e.preventDefault(); }
  else if (e.key === 'Enter') { const it = pItems[pCursor]; if (it) switchProject(it.p.path); e.preventDefault(); }
  else if (e.key === 'Escape') { closeProjects(); e.preventDefault(); e.stopPropagation(); }
});
$('plist').addEventListener('click', e => {
  const el = e.target.closest('.ref[data-i]');
  if (el) switchProject(pItems[+el.dataset.i].p.path);
});
$('limit').addEventListener('change', e => { store.set('limit', e.target.value); loadLog(); });
$('reload').addEventListener('click', () => reload({ anchor: true, fresh: true }));
$('cols').addEventListener('mousedown', e => {
  const grip = e.target.closest('.grip');
  if (!grip) return;
  e.preventDefault();
  const col = grip.dataset.col, dir = +grip.dataset.dir, x0 = e.clientX;
  const w0 = col === 'graph' ? graphWidth() : parseFloat(getComputedStyle($('pane')).getPropertyValue(`--w-${col}`));
  const [min, max] = COLS[col];
  const move = ev => {
    colWidths[col] = Math.round(Math.max(min, Math.min(max, w0 + dir * (ev.clientX - x0))));
    applyCols();
    render();
  };
  const up = () => {
    window.removeEventListener('mousemove', move);
    window.removeEventListener('mouseup', up);
    prefs.set('cols', colWidths);
  };
  window.addEventListener('mousemove', move);
  window.addEventListener('mouseup', up);
});
$('cols').addEventListener('dblclick', e => {
  const grip = e.target.closest('.grip');
  if (!grip) return;
  delete colWidths[grip.dataset.col];
  prefs.set('cols', colWidths);
  applyCols();
  render();
});
$('fetch').addEventListener('click', fetchRemotes);
$('auto-fetch').addEventListener('change', e => {
  store.set('autoFetch', e.target.checked);
  autoFetchTick();
});
document.addEventListener('visibilitychange', () => { poll(); autoFetchTick(); });
$('close').addEventListener('click', closeDetail);
$('files').addEventListener('click', e => {
  const el = e.target.closest('.file[data-id]');
  const sec = el && document.getElementById(el.dataset.id);
  if (!sec) return;
  sec.classList.remove('closed');
  updateFoldAll();
  const diff = $('diff');
  diff.scrollTop += sec.getBoundingClientRect().top - diff.getBoundingClientRect().top;
});
$('diff').addEventListener('click', e => {
  const fh = e.target.closest('.fsec > .fh');
  if (!fh) return;
  fh.parentElement.classList.toggle('closed');
  updateFoldAll();
});
$('fold-all').addEventListener('click', () => {
  const sections = [...$('diff').querySelectorAll('.fsec')];
  const fold = sections.some(s => !s.classList.contains('closed'));
  sections.forEach(s => s.classList.toggle('closed', fold));
  updateFoldAll();
});
function openMenubar(mb) {
  closeMenubar();
  closeMenu();
  mb.classList.add('open');
  mb.querySelector('.mb-menu').hidden = false;
  mb.querySelector('[data-cmd="close"]')?.toggleAttribute('disabled', !repo.path);
}

function closeMenubar() {
  const open = $('mainmenu').querySelector('.mb.open');
  if (!open) return false;
  open.classList.remove('open');
  open.querySelector('.mb-menu').hidden = true;
  return true;
}

$('proj-chip').addEventListener('click', () => $('pmenu').hidden ? openProjects() : closeProjects());
$('mainmenu').addEventListener('click', e => {
  const btn = e.target.closest('.mb-btn');
  if (btn) {
    const mb = btn.parentElement;
    if (mb.classList.contains('open')) closeMenubar();
    else openMenubar(mb);
    return;
  }
  const cmd = e.target.closest('[data-cmd]')?.dataset.cmd;
  const size = prefs.get('fontSize') ?? FONT_SIZES.default;
  if (cmd === 'open' || cmd === 'close') closeMenubar();
  if (cmd === 'open') openProjects();
  if (cmd === 'close') closeProject();
  if (cmd === 'smaller') setFontSize(size - 1);
  if (cmd === 'bigger') setFontSize(size + 1);
  if (cmd === 'reset-size') setFontSize(FONT_SIZES.default);
  const theme = e.target.closest('#theme-seg [data-theme]')?.dataset.theme;
  if (theme) setTheme(theme);
});
$('split').addEventListener('mousedown', e => {
  e.preventDefault();
  const move = ev => { $('detail').style.width = `${Math.max(240, window.innerWidth - ev.clientX)}px`; render(); };
  const up = () => {
    window.removeEventListener('mousemove', move);
    window.removeEventListener('mouseup', up);
    store.set('detailWidth', $('detail').offsetWidth);
  };
  window.addEventListener('mousemove', move);
  window.addEventListener('mouseup', up);
});
document.addEventListener('keydown', e => {
  if (e.target.closest('input, select') || e.ctrlKey || e.metaKey || e.altKey) return;
  const page = Math.max(1, Math.floor($('list').clientHeight / ROW) - 1);
  const keys = {
    ArrowDown: () => setCursor(cursor + 1), j: () => setCursor(cursor + 1),
    ArrowUp: () => setCursor(cursor - 1), k: () => setCursor(cursor - 1),
    PageDown: () => setCursor(cursor + page), PageUp: () => setCursor(cursor - page),
    Home: () => setCursor(0), End: () => setCursor(commits.length - 1),
    Enter: () => setCursor(cursor < 0 ? 0 : cursor, { open: true }),
    Escape: () => {
      if (closeMenubar()) return;
      if (!$('ctx').hidden) return closeContext();
      if (!$('menu').hidden) closeMenu();
      else closeDetail();
    },
    b: openMenu, '/': openMenu, p: openProjects, r: () => reload({ anchor: true, fresh: true }), f: fetchRemotes, c: () => commits[cursor] && copyHash(commits[cursor].h),
  };
  if (keys[e.key]) { keys[e.key](); e.preventDefault(); }
});

init().catch(e => { $('rows').innerHTML = `<div id="error">${esc(e.message)}</div>`; });
