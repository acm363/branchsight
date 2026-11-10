import { getJson } from '../shared/api.js';
import { prefs } from '../shared/prefs.js';
import { $, copy, drag, toast } from '../shared/dom.js';
import { SHORT_HASH, esc, fileLabel, plural, statusClass } from '../shared/format.js';
import { t } from '../shared/i18n.js';
import { refName } from '../history/labels.js';
import * as log from '../history/list.js';
import { lineHtml, patchHtml } from './patch.js';
import * as edit from '../actions/edit.js';
import { commitAt, isWorktree, state } from '../shared/state.js';
import { onLog } from '../history/sync.js';
import * as viewer from './viewer.js';
import { formatRef, showRefMenu } from './copyref.js';

const MAX_LINES = 40000;

let shown = null;
let requested = null;
let drawnWorktree = null;
let timer = 0;

export const isOpen = () => !$('detail').hidden;

export function showSoon(entry, delay) {
  clearTimeout(timer);
  timer = setTimeout(() => (isWorktree(entry) ? showWorktree() : showCommit(entry)), delay);
}

export function close() {
  clearTimeout(timer);
  requested = shown = null;
  state.compareHash = null;
  $('detail').hidden = $('split').hidden = $('commit-btn').hidden = true;
  requestAnimationFrame(log.render);
}

function clearCompare() {
  if (!state.compareHash) return;
  state.compareHash = null;
  log.render();
}

const hashSpan = (hash, length = 10) => `<span class="h" data-h="${hash}" title="${esc(t('panel.copyHash', { hash }))}">${hash.slice(0, length)}</span>`;

function sideLabel(hash) {
  const commit = state.entries.find(e => e.h === hash);
  const name = refName(state.refsByHash.get(hash));
  return `${name ? `${name} · ` : ''}${hash.slice(0, SHORT_HASH)}${commit ? ` ${commit.s}` : ''}`;
}

function showCommit(commit) {
  clearCompare();
  const parent = commit.p[0];
  const sides = [parent ? t('panel.parent', { label: sideLabel(parent) }) : t('panel.firstCommit'), sideLabel(commit.h)];
  const source = { kind: 'commit', key: commit.h, h: commit.h };
  return open(source, `${hashSpan(commit.h)}${esc(commit.s)}`, commit.s, async budget => {
    const changes = await getJson('/api/commit', { h: commit.h });
    return [{ header: changes.header, ...group('', changes, 'commit', sides, budget) }];
  });
}

export function showCompare(i, j) {
  const a = commitAt(i), b = commitAt(j);
  if (!a || !b) return toast(t('panel.compareTwo'), 3000);
  const [newer, older] = i < j ? [a, b] : [b, a];
  state.compareHash = b.h;
  log.render();
  const source = { kind: 'compare', key: `${older.h}..${newer.h}`, from: older.h, to: newer.h };
  const title = `<span class="cmp-label">${esc(t('panel.compare'))}</span>${hashSpan(older.h, SHORT_HASH)}<span class="arrow">→</span>${hashSpan(newer.h, SHORT_HASH)}`;
  return open(source, title, `${older.s} → ${newer.s}`, async budget => {
    const changes = await getJson('/api/compare', { from: older.h, to: newer.h });
    return [group('', changes, 'compare', [sideLabel(older.h), sideLabel(newer.h)], budget)];
  });
}

export function showWorktree({ keep = false } = {}) {
  if (!keep) drawnWorktree = null;
  clearCompare();
  const head = `HEAD${state.repo.branch ? ` · ${state.repo.branch}` : ''}`;
  const source = { kind: 'worktree', key: 'worktree' };
  return open(source, `<span class="wt-title">${esc(t('wt.uncommitted'))}</span>`, t('wt.uncommitted'), async budget => {
    const worktree = await getJson('/api/worktree');
    const untracked = worktree.untracked.map((path, i) => ({
      status: '?', path, add: null, del: null, kind: 'untracked', id: `untracked-${i}`, body: '', sides: [t('panel.newFile'), t('panel.workingTree')],
    }));
    return [
      group(t('panel.staged'), worktree.staged, 'staged', [head, t('panel.stagedIndex')], budget),
      group(t('panel.notStaged'), worktree.unstaged, 'unstaged', [t('panel.stagedIndex'), t('panel.workingTree')], budget),
      { title: t('panel.untracked'), kind: 'untracked', files: untracked, note: '' },
    ].filter(g => g.files.length);
  }, keep);
}

function group(title, { files, truncated }, kind, sides, budget) {
  let hidden = 0;
  const conflictSides = [t('panel.ourSide'), sides[1]];
  const listed = files.map(({ patch, ...file }, i) => {
    const rendered = patchHtml(patch, budget.lines);
    budget.lines -= rendered.shown;
    hidden += rendered.hidden;
    const fileSides = kind === 'unstaged' && file.status === 'U' ? conflictSides : sides;
    return { ...file, kind, sides: fileSides, id: `${kind}-${i}`, body: rendered.html };
  });
  const notes = [hidden && t('panel.moreNotShown', { lines: plural(hidden, 'more line') }), truncated && t('panel.diffCut')].filter(Boolean);
  return { title, kind, files: listed, note: notes.length ? t('panel.notesSuffix', { notes: notes.join(', ') }) : '' };
}

const STAGE = { staged: 'unstage', unstaged: 'stage', untracked: 'stage' };

function fileActions(f) {
  const verb = STAGE[f.kind];
  return `<button class="fact" data-act="${verb}" title="${esc(t(`panel.${verb}File`))}">${esc(t(`verb.${verb}`))}</button>`
    + `<button class="fact danger" data-act="discard" title="${esc(t('panel.discardFile'))}">${esc(t('verb.discard'))}</button>`;
}

function groupActions(g) {
  const verb = STAGE[g.kind];
  return `<button class="gact" data-act="${verb}-group" data-kind="${g.kind}">${esc(t(`panel.${verb}All`))}</button>`
    + `<button class="gact danger" data-act="discard-group" data-kind="${g.kind}">${esc(t('panel.discardAll'))}</button>`;
}

function fileAction(act, key) {
  const path = key.slice(key.indexOf(':') + 1);
  ({ stage: edit.stage, unstage: edit.unstage, discard: edit.discard }[act])([path]);
}

function groupAction(act, kind) {
  const paths = shown.files.filter(f => f.kind === kind).map(f => f.path);
  if (!paths.length) return;
  if (act === 'stage-group') edit.stage(paths);
  else if (act === 'unstage-group') edit.unstage(paths);
  else edit.discard(paths);
}

async function open(source, titleHtml, title, load, keep = false) {
  requested = source.key;
  if (!keep) {
    shown = null;
    treeCollapsed.clear();
  }
  $('detail').hidden = $('split').hidden = false;
  $('detail-title').innerHTML = titleHtml;
  $('detail-title').title = title;
  $('open-viewer').disabled = !shown;
  try {
    const groups = await load({ lines: MAX_LINES });
    if (requested !== source.key) return false;
    const folds = keep ? keptFolds() : null;
    shown = { source, files: groups.flatMap(g => g.files) };
    fill(groups, folds);
    return true;
  } catch (e) {
    if (requested !== source.key) return false;
    shown = null;
    $('files').innerHTML = '';
    $('files-count').textContent = '';
    $('diff').innerHTML = `<div id="error">${esc(e.message)}</div>`;
    return false;
  }
}

const fileKey = f => `${f.kind}:${f.path}`;

function counts(f) {
  if (f.kind === 'untracked') return `<span class="n">${esc(t('panel.new'))}</span>`;
  if (f.status === 'U') return `<span class="n">${esc(t('panel.conflict'))}</span>`;
  if (f.add === null) return `<span class="n">${esc(t('panel.binary'))}</span>`;
  return `<span class="n"><b>+${f.add}</b> <i>−${f.del}</i></span>`;
}

const treeCollapsed = new Set();

function treeOf(files) {
  const root = { dirs: new Map(), files: [] };
  for (const f of files) {
    const parts = f.path.split('/');
    const name = parts.pop();
    let node = root;
    for (const part of parts) {
      if (!node.dirs.has(part)) node.dirs.set(part, { dirs: new Map(), files: [] });
      node = node.dirs.get(part);
    }
    node.files.push({ file: f, name });
  }
  const out = [];
  renderNode(root, '', 0, out);
  return out.join('');
}

function renderNode(node, prefix, depth, out) {
  for (const [dirName, dirNode] of node.dirs) {
    let name = dirName, path = prefix ? `${prefix}/${dirName}` : dirName, dir = dirNode;
    while (dir.dirs.size === 1 && !dir.files.length) {
      const [childName, child] = dir.dirs.entries().next().value;
      name += ` / ${childName}`;
      path += `/${childName}`;
      dir = child;
    }
    const collapsed = treeCollapsed.has(path);
    out.push(`<div class="fdir${collapsed ? ' collapsed' : ''}" data-folder="${esc(path)}" style="--d:${depth}"><span class="tcaret">▾</span><span class="tname">${esc(name)}</span></div>`);
    out.push(`<div class="fkids${collapsed ? ' hidden' : ''}">`);
    renderNode(dir, path, depth + 1, out);
    out.push('</div>');
  }
  for (const { file, name } of node.files) {
    out.push(`<div class="file" data-id="${file.id}" style="--d:${depth}" title="${esc(fileLabel(file))}">`
      + `<span class="st ${statusClass(file)}">${file.status}</span><span class="p">${esc(name)}</span>${counts(file)}</div>`);
  }
}

function toggleFolder(dir) {
  const collapsed = dir.classList.toggle('collapsed');
  if (collapsed) treeCollapsed.add(dir.dataset.folder);
  else treeCollapsed.delete(dir.dataset.folder);
  const kids = dir.nextElementSibling;
  if (kids?.classList.contains('fkids')) kids.classList.toggle('hidden', collapsed);
}

function keptFolds() {
  const closed = new Set([...$('diff').querySelectorAll('.fsec.closed')].map(s => s.dataset.key));
  return { closed, diff: $('diff').scrollTop, files: $('files').scrollTop };
}

function fill(groups, folds) {
  const titled = groups.length > 1 || Boolean(groups[0]?.title);
  const editable = shown.source.kind === 'worktree';
  const untracked = f => f.kind === 'untracked';
  const label = f => esc(fileLabel(f));
  const section = f => {
    const closed = untracked(f) || folds?.closed.has(fileKey(f));
    return `<div class="fsec${closed ? ' closed' : ''}${untracked(f) ? ' untracked' : ''}" id="${f.id}" data-key="${esc(fileKey(f))}">`
      + `<div class="fh"><span class="st ${statusClass(f)}">${f.status}</span><span class="p">${label(f)}</span>${counts(f)}`
      + `${editable ? fileActions(f) : ''}<button class="open-v" title="${esc(t('panel.openInViewer'))}">⤢</button>`
      + `${untracked(f) ? '' : `<button class="fold" title="${esc(t('panel.foldFile'))}">▾</button>`}</div><div class="fbody">${f.body}</div></div>`;
  };
  const header = groups[0]?.header?.replace(/\n$/, '').split('\n').map(line => lineHtml(line)).join('') ?? '';
  $('commit-btn').hidden = !editable;
  $('diff').innerHTML = header + groups.map(g => (titled ? `<div class="grp">${esc(g.title)} · ${g.files.length}${editable && g.kind ? groupActions(g) : ''}</div>` : '')
    + g.files.map(section).join('') + (g.note ? lineHtml(g.note, 'mt') : '')).join('');
  $('files').innerHTML = groups.map(g => (titled ? `<div class="file-grp">${esc(g.title)}</div>` : '') + treeOf(g.files)).join('')
    || `<div class="file"><span class="p">${esc(t('panel.noFileChanges'))}</span></div>`;
  $('diff').scrollTop = folds?.diff ?? 0;
  $('files').scrollTop = folds?.files ?? 0;
  $('files-count').textContent = plural(shown.files.length, 'file');
  $('open-viewer').disabled = !shown.files.length;
  updateFoldAll();
  activeFile = null;
  markCurrentFile();
}

function updateFoldAll() {
  const sections = [...$('diff').querySelectorAll('.fsec:not(.untracked)')];
  $('fold-all').textContent = sections.some(s => !s.classList.contains('closed')) ? t('btn.collapseAll') : t('btn.expandAll');
  $('fold-all').disabled = !sections.length;
}

const SPY_EDGE = 8;
let activeFile = null;
let spyPending = false;

function spyScroll() {
  if (spyPending) return;
  spyPending = true;
  requestAnimationFrame(() => { spyPending = false; markCurrentFile(); });
}

function markCurrentFile() {
  if (!shown) return;
  const diff = $('diff'), secs = diff.querySelectorAll('.fsec');
  if (!secs.length) return;
  let current;
  if (diff.scrollTop + diff.clientHeight >= diff.scrollHeight - 1) {
    current = secs[secs.length - 1];
  } else {
    const edge = diff.getBoundingClientRect().top + SPY_EDGE;
    for (const sec of secs) {
      if (sec.getBoundingClientRect().top <= edge) current = sec;
      else break;
    }
    current ??= secs[0];
  }
  const id = current.id;
  if (id === activeFile) return;
  activeFile = id;
  const files = $('files');
  files.querySelector('.file.current')?.classList.remove('current');
  const entry = files.querySelector(`.file[data-id="${CSS.escape(id)}"]`);
  if (!entry) return;
  entry.classList.add('current');
  if (!entry.offsetParent) return;
  const r = entry.getBoundingClientRect(), box = files.getBoundingClientRect();
  if (r.top < box.top) files.scrollTop += r.top - box.top;
  else if (r.bottom > box.bottom) files.scrollTop += r.bottom - box.bottom;
}

function openViewer(id) {
  if (!shown) return;
  viewer.open(shown.source, shown.files, Math.max(0, shown.files.findIndex(f => f.id === id)));
}

onLog(seen => {
  if (requested !== 'worktree') return;
  if (!isWorktree(state.entries[0])) return close();
  const drawn = `${seen?.worktree}|${state.repo.head}|${state.repo.branch}`;
  if (drawn === drawnWorktree) return;
  drawnWorktree = drawn;
  showWorktree({ keep: true }).then(ok => {
    if (!ok) drawnWorktree = null;
  });
});

function lineMap(fbody) {
  const map = new Map();
  let oldLine = 0, newLine = 0;
  for (const el of fbody.children) {
    const cls = el.className;
    if (cls === 'hk') {
      const m = /@@ -(\d+)(?:,\d+)? \+(\d+)/.exec(el.textContent);
      if (m) [oldLine, newLine] = [+m[1], +m[2]];
    } else if (cls === 'ad') map.set(el, { line: newLine++, kind: 'add', side: 'new' });
    else if (cls === 'rm') map.set(el, { line: oldLine++, kind: 'del', side: 'old' });
    else if (cls !== 'mt') { map.set(el, { line: newLine, kind: 'ctx', side: 'new' }); oldLine++; newLine++; }
  }
  return map;
}

function lineSpan(node, fbody) {
  let el = node.nodeType === 3 ? node.parentElement : node;
  while (el && el.parentElement !== fbody) el = el.parentElement;
  return el && el.parentElement === fbody ? el : null;
}

function pointAt(map, span, node, offset) {
  const info = map.get(span);
  if (!info) return null;
  const range = document.createRange();
  range.setStart(span, 0);
  range.setEnd(node, offset);
  return { line: info.line, col: Math.max(0, range.toString().length - 1) + 1, side: info.side };
}

function codeForRange(map, fbody, startSpan, endSpan, side) {
  const spans = [...fbody.children];
  const i = spans.indexOf(startSpan);
  let j = spans.indexOf(endSpan);
  if (i < 0) return '';
  if (j < i) j = i;
  return spans.slice(i, j + 1).map(span => {
    const info = map.get(span);
    if (!info || (side === 'new' && info.kind === 'del') || (side === 'old' && info.kind === 'add')) return null;
    return span.textContent.slice(1);
  }).filter(text => text != null).join('\n');
}

function diffReference(e) {
  const fbody = e.target.closest?.('.fbody');
  if (!fbody || !shown) return null;
  const key = fbody.closest('.fsec')?.dataset.key ?? '';
  const path = key.slice(key.indexOf(':') + 1);
  if (!path) return null;
  const map = lineMap(fbody), sel = getSelection();
  let a = null, b = null, code = '';
  if (sel.rangeCount && !sel.isCollapsed && fbody.contains(sel.anchorNode) && fbody.contains(sel.focusNode)) {
    const r = sel.getRangeAt(0);
    const startSpan = lineSpan(r.startContainer, fbody), endSpan = lineSpan(r.endContainer, fbody);
    if (startSpan) {
      a = pointAt(map, startSpan, r.startContainer, r.startOffset);
      b = endSpan ? pointAt(map, endSpan, r.endContainer, r.endOffset) : null;
      if (a) code = codeForRange(map, fbody, startSpan, endSpan ?? startSpan, a.side);
    }
  }
  if (!a) {
    const span = lineSpan(e.target, fbody), info = span && map.get(span);
    if (!info) return null;
    a = { line: info.line };
    code = span.textContent.slice(1);
  }
  return { text: formatRef(path, a, b, shown.source), code: code.replace(/\s+$/, '') };
}

$('diff').addEventListener('contextmenu', e => showRefMenu(e, diffReference(e)));

$('diff').addEventListener('scroll', spyScroll);

$('close').addEventListener('click', close);

$('detail-title').addEventListener('click', e => {
  const hash = e.target.closest('.h[data-h]')?.dataset.h;
  if (hash) copy(hash);
});

$('files').addEventListener('click', e => {
  const dir = e.target.closest('.fdir');
  if (dir) return toggleFolder(dir);
  const section = document.getElementById(e.target.closest('.file[data-id]')?.dataset.id ?? '');
  if (!section) return;
  section.classList.remove('closed');
  updateFoldAll();
  const diff = $('diff');
  diff.scrollTop += section.getBoundingClientRect().top - diff.getBoundingClientRect().top;
});

$('files').addEventListener('dblclick', e => {
  const id = e.target.closest('.file[data-id]')?.dataset.id;
  if (id) openViewer(id);
});

$('diff').addEventListener('click', e => {
  const groupBtn = e.target.closest('.gact');
  if (groupBtn) return groupAction(groupBtn.dataset.act, groupBtn.dataset.kind);
  const section = e.target.closest('.fsec');
  if (!section) return;
  const fileBtn = e.target.closest('.fact');
  if (fileBtn) return fileAction(fileBtn.dataset.act, section.dataset.key);
  if (e.target.closest('.open-v')) return openViewer(section.id);
  if (!e.target.closest('.fh') || section.classList.contains('untracked')) return;
  section.classList.toggle('closed');
  updateFoldAll();
});

$('commit-btn').addEventListener('click', () => edit.commit());

$('fold-all').addEventListener('click', () => {
  const sections = [...$('diff').querySelectorAll('.fsec:not(.untracked)')];
  const fold = sections.some(s => !s.classList.contains('closed'));
  sections.forEach(s => s.classList.toggle('closed', fold));
  updateFoldAll();
});

$('open-viewer').addEventListener('click', () => openViewer(shown?.files[0]?.id));

const dockedBottom = () => $('content').classList.contains('dock-bottom');

export function applyDock() {
  const bottom = prefs.get('dock') === 'bottom';
  $('content').classList.toggle('dock-bottom', bottom);
  const detail = $('detail'), files = $('files');
  if (bottom) {
    detail.style.width = files.style.height = '';
    const h = state.store.get('detailHeight'), fw = state.store.get('filesWidth');
    detail.style.height = h ? `${h}px` : '';
    files.style.width = fw ? `${fw}px` : '';
  } else {
    detail.style.height = files.style.width = '';
    const w = state.store.get('detailWidth'), fh = state.store.get('filesHeight');
    detail.style.width = w ? `${w}px` : '';
    files.style.height = fh ? `${fh}px` : '';
  }
}

$('split').addEventListener('mousedown', e => drag(e, ev => {
  if (dockedBottom()) {
    $('detail').style.height = `${Math.max(140, Math.min(window.innerHeight - ev.clientY, $('content').clientHeight - 80))}px`;
  } else {
    $('detail').style.width = `${Math.max(240, $('content').getBoundingClientRect().right - ev.clientX)}px`;
  }
}, () => state.store.set(dockedBottom() ? 'detailHeight' : 'detailWidth', dockedBottom() ? $('detail').offsetHeight : $('detail').offsetWidth)));

$('files-split').addEventListener('mousedown', e => drag(e, ev => {
  const box = $('files').getBoundingClientRect();
  if (dockedBottom()) {
    $('files').style.width = `${Math.max(120, Math.min(ev.clientX - box.left, $('detail').clientWidth - 160))}px`;
  } else {
    $('files').style.height = `${Math.max(48, Math.min(ev.clientY - box.top, $('detail').clientHeight - 140))}px`;
  }
}, () => state.store.set(dockedBottom() ? 'filesWidth' : 'filesHeight', dockedBottom() ? $('files').offsetWidth : $('files').offsetHeight)));
