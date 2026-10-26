import { DEFAULT_SELECTION, GROUPS, layout } from './graph.js';

const ROW = 20, LANE = 16, MAX_LANES = 30, MAX_DIFF_LINES = 40000;
const POLL_MS = 5000, AUTO_FETCH_MS = 120000;
const COLORS = ['#3b82f6', '#e5484d', '#30a46c', '#f5a524', '#8e4ec6', '#12a594', '#d6409f', '#0090ff', '#ad7f58', '#7c66dc', '#e16f24', '#5bb98b'];
const $ = id => document.getElementById(id);
const esc = s => s.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const color = n => COLORS[n % COLORS.length];

let repo = {}, refs = [], selected = [], commits = [], rows = [], lanes = 1, cursor = -1, openHash = null;
let refsByHash = new Map(), menuItems = [], menuCursor = 0, logTimer = 0, detailTimer = 0;
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

async function api(path) {
  const r = await fetch(path);
  if (!r.ok) throw new Error(await r.text());
  return r;
}

function fmtDate(t) {
  const d = new Date(t * 1000), p = n => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

const graphWidth = () => 18 + Math.min(lanes, MAX_LANES) * LANE;

function chipsFor(c) {
  const list = refsByHash.get(c.h) || [];
  return list.map(r => {
    const head = r.kind === 'head' && r.name === repo.branch;
    return `<span class="chip k-${r.kind}${head ? ' is-head' : ''}">${esc(r.name)}</span>`;
  }).join('') + (c.h === repo.head && !repo.branch ? '<span class="chip is-head">HEAD</span>' : '');
}

function render() {
  const list = $('list'), top = list.scrollTop, h = list.clientHeight;
  const first = Math.max(0, Math.floor(top / ROW) - 4);
  const last = Math.min(commits.length, Math.ceil((top + h) / ROW) + 4);
  const gw = graphWidth();
  let html = '';
  for (let i = first; i < last; i++) {
    const c = commits[i];
    html += `<div class="row${i === cursor ? ' sel' : ''}${c.p.length > 1 ? ' merge' : ''}" data-i="${i}" style="top:${i * ROW}px;padding-left:${gw}px">`
      + `<span class="s">${chipsFor(c)}<span class="t">${esc(c.s)}</span></span>`
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
  g.lineWidth = 2.4;
  g.lineCap = 'round';
  const X = j => 9 + j * LANE;
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
    g.arc(X(r.col), mid, c.p.length > 1 ? 4 : 4.8, 0, Math.PI * 2);
    g.fillStyle = c.p.length > 1 ? bg : color(r.color);
    g.strokeStyle = color(r.color);
    g.fill();
    g.stroke();
    if (c.h === repo.head) {
      g.beginPath();
      g.arc(X(r.col), mid, 7.8, 0, Math.PI * 2);
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
  let html = '', inDiff = false, file = null;
  lines.slice(0, MAX_DIFF_LINES).forEach(line => {
    let cls = '';
    if (line.startsWith('diff --git ')) {
      inDiff = true;
      file = { path: diffPath(line), st: 'M', add: 0, del: 0, id: `f${files.length}` };
      files.push(file);
      html += `<span class="fh" id="${file.id}">${esc(file.path)}</span>`;
      return;
    }
    if (inDiff && file) {
      if (/^(new file mode)/.test(line)) file.st = 'A';
      else if (/^(deleted file mode)/.test(line)) file.st = 'D';
      else if (/^rename from /.test(line)) { file.st = 'R'; file.from = line.slice(12); }
      if (line.startsWith('@@')) cls = 'hk';
      else if (line.startsWith('+++') || line.startsWith('---') || /^(index |new file|deleted file|similarity|dissimilarity|rename |copy |old mode|new mode|Binary files)/.test(line)) cls = 'mt';
      else if (line[0] === '+') { cls = 'ad'; file.add++; }
      else if (line[0] === '-') { cls = 'rm'; file.del++; }
    }
    html += `<span${cls ? ` class="${cls}"` : ''}>${esc(line) || ' '}</span>`;
  });
  if (lines.length > MAX_DIFF_LINES) html += `<span class="mt">… ${lines.length - MAX_DIFF_LINES} more lines</span>`;
  $('diff').innerHTML = html;
  $('diff').scrollTop = 0;
  $('files').innerHTML = files.map(f => `<div class="file" data-id="${f.id}" title="${esc(f.from ? `${f.from} → ${f.path}` : f.path)}">`
    + `<span class="st st-${f.st}">${f.st}</span><span class="p">${esc(f.from ? `${f.from} → ${f.path}` : f.path)}</span>`
    + `<span class="n"><b>+${f.add}</b> <i>−${f.del}</i></span></div>`).join('') || '<div class="file"><span class="p">no file changes</span></div>';
  render();
}

function diffPath(line) {
  const s = line.slice(11);
  const half = (s.length - 1) / 2;
  if (Number.isInteger(half) && s.slice(2, half) === s.slice(half + 3)) return s.slice(half + 3);
  const i = s.lastIndexOf(' b/');
  return i >= 0 ? s.slice(i + 3) : s;
}

function closeDetail() {
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
  clearTimeout(logTimer);
  logTimer = setTimeout(loadLog, 200);
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
  selected = selected.filter(n => names.has(n));
  renderChips();
}

async function loadLog({ anchor = false } = {}) {
  const params = new URLSearchParams(selected.map(n => ['ref', n]));
  params.set('limit', $('limit').value);
  const keep = commits[cursor]?.h;
  const list = $('list'), topIndex = Math.floor(list.scrollTop / ROW);
  const top = anchor ? { h: commits[topIndex]?.h, offset: list.scrollTop - topIndex * ROW } : null;
  if (!anchor) $('count').textContent = 'loading…';
  try {
    commits = await (await api(withRepo('/api/log', params))).json();
  } catch (e) {
    commits = [];
    loadFailed = true;
    $('count').textContent = '';
    $('rows').innerHTML = `<div id="error">${esc(e.message)} — retrying…</div>`;
    return;
  }
  loadFailed = false;
  ({ rows, lanes } = layout(commits));
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

async function reload({ anchor = false } = {}) {
  repo = await (await api(withRepo('/api/repo'))).json();
  await loadRefs();
  await loadLog({ anchor });
}

let version = null, polling = false, fetching = false, lastFetch = 0, loadFailed = false;

async function poll() {
  if (!repo.path || document.hidden || polling) return;
  polling = true;
  try {
    const { v } = await (await api(withRepo('/api/state'))).json();
    if (loadFailed || (version !== null && v !== version)) await reload({ anchor: true });
    version = v;
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
    const r = await fetch(withRepo('/api/fetch'), { method: 'POST' });
    if (!r.ok) throw new Error(await r.text());
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
  prefs.set('last', path);
  location.href = `${location.pathname}?repo=${encodeURIComponent(path)}`;
}

async function resolveRepo() {
  const params = new URLSearchParams(location.search);
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
  ctx = await (await api('/api/context')).json();
  repoPath = await resolveRepo();
  if (repoPath) {
    try {
      repo = await (await api(withRepo('/api/repo'))).json();
    } catch (e) {
      $('rows').innerHTML = `<div id="error">${esc(e.message)}</div>`;
      repoPath = null;
    }
  }
  if (!repoPath) {
    $('repo').textContent = 'choose a project';
    return openProjects();
  }
  repoPath = repo.path;
  prefs.set('last', repo.path);
  prefs.set('recent', [repo.path, ...(prefs.get('recent') || []).filter(p => p !== repo.path)].slice(0, 30));
  document.title = `${repo.name} · BranchSight`;
  $('repo').textContent = repo.name;
  $('repo').title = repo.path;
  const fromUrl = new URLSearchParams(location.search).getAll('ref');
  selected = fromUrl.length ? fromUrl : store.get('sel') ?? DEFAULT_SELECTION;
  $('opt-remote').checked = store.get('remote') ?? true;
  $('opt-tag').checked = store.get('tag') ?? false;
  $('limit').value = store.get('limit') ?? '5000';
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
let toastTimer = 0;
async function copyHash(h) {
  try {
    await navigator.clipboard.writeText(h);
  } catch {
    const ta = Object.assign(document.createElement('textarea'), { value: h });
    document.body.append(ta);
    ta.select();
    document.execCommand('copy');
    ta.remove();
  }
  showToast(`copied ${h}`);
}

function showToast(text, ms = 1400) {
  $('toast').textContent = text;
  $('toast').classList.add('on');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => $('toast').classList.remove('on'), ms);
}

$('rows').addEventListener('click', e => {
  const row = e.target.closest('.row');
  if (!row) return;
  if (e.target.closest('.h')) {
    copyHash(commits[+row.dataset.i].h);
    setCursor(+row.dataset.i, { scroll: false });
  } else setCursor(+row.dataset.i, { scroll: false, open: true });
});
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
  if (!$('menu').hidden && !$('picker').contains(e.target)) closeMenu();
  if (!$('pmenu').hidden && !$('projects').contains(e.target)) closeProjects();
});
$('proj-btn').addEventListener('click', () => $('pmenu').hidden ? openProjects() : closeProjects());
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
$('reload').addEventListener('click', () => reload({ anchor: true }));
$('fetch').addEventListener('click', fetchRemotes);
$('auto-fetch').addEventListener('change', e => {
  store.set('autoFetch', e.target.checked);
  autoFetchTick();
});
document.addEventListener('visibilitychange', () => { poll(); autoFetchTick(); });
$('close').addEventListener('click', closeDetail);
$('files').addEventListener('click', e => {
  const el = e.target.closest('.file[data-id]');
  if (el) document.getElementById(el.dataset.id)?.scrollIntoView({ block: 'start' });
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
    Escape: () => $('menu').hidden ? closeDetail() : closeMenu(),
    b: openMenu, '/': openMenu, p: openProjects, r: () => reload({ anchor: true }), f: fetchRemotes, c: () => commits[cursor] && copyHash(commits[cursor].h),
  };
  if (keys[e.key]) { keys[e.key](); e.preventDefault(); }
});

init().catch(e => { $('rows').innerHTML = `<div id="error">${esc(e.message)}</div>`; });
