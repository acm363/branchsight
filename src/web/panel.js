import { getJson } from './api.js';
import { $, copy, drag, toast } from './dom.js';
import { esc, plural } from './format.js';
import { refName } from './labels.js';
import * as log from './log.js';
import { patchSections } from './patch.js';
import { commitAt, isWorktree, state } from './state.js';
import * as viewer from './viewer.js';

const MAX_LINES = 40000;

let shown = null;
let requested = null;
let timer = 0;

export const isOpen = () => !$('detail').hidden;

export const showingWorktree = () => requested === 'worktree';

export function showSoon(entry, delay) {
  clearTimeout(timer);
  timer = setTimeout(() => (isWorktree(entry) ? showWorktree() : showCommit(entry)), delay);
}

export function close() {
  clearTimeout(timer);
  requested = shown = null;
  state.compareHash = null;
  $('detail').hidden = $('split').hidden = true;
  requestAnimationFrame(log.render);
}

const hashSpan = (hash, length = 10) => `<span class="h" data-h="${hash}" title="Copy ${hash}">${hash.slice(0, length)}</span>`;

function sideLabel(hash) {
  const commit = state.entries.find(e => e.h === hash);
  const name = refName(state.refsByHash.get(hash));
  return `${name ? `${name} · ` : ''}${hash.slice(0, 7)}${commit ? ` ${commit.s}` : ''}`;
}

function showCommit(commit) {
  state.compareHash = null;
  const parent = commit.p[0];
  const from = parent ? `parent · ${sideLabel(parent)}` : 'nothing (first commit)';
  const source = { kind: 'commit', key: commit.h, h: commit.h, labels: [from, sideLabel(commit.h)] };
  return open(source, `${hashSpan(commit.h)}${esc(commit.s)}`, commit.s, async () => {
    const changes = await getJson('/api/commit', { h: commit.h });
    return [{ header: changes.header, ...group('', changes, 'commit', 'c') }];
  });
}

export function showCompare(i, j) {
  const a = commitAt(i), b = commitAt(j);
  if (!a || !b) return toast('compare two commits, not the uncommitted changes', 3000);
  const [newer, older] = i < j ? [a, b] : [b, a];
  state.compareHash = b.h;
  log.render();
  const source = { kind: 'compare', key: `${older.h}..${newer.h}`, from: older.h, to: newer.h, labels: [sideLabel(older.h), sideLabel(newer.h)] };
  const title = `<span class="cmp-label">Compare</span>${hashSpan(older.h, 7)}<span class="arrow">→</span>${hashSpan(newer.h, 7)}`;
  return open(source, title, `${older.s} → ${newer.s}`, async () => {
    const changes = await getJson('/api/compare', { from: older.h, to: newer.h });
    return [group('', changes, 'compare', 'x')];
  });
}

export function showWorktree({ keep = false } = {}) {
  state.compareHash = null;
  const source = { kind: 'worktree', key: 'worktree' };
  return open(source, '<span class="wt-title">Uncommitted changes</span>', 'Uncommitted changes', async () => {
    const worktree = await getJson('/api/worktree');
    const untracked = worktree.untracked.map((path, i) => ({ status: 'U', path, add: null, del: null, kind: 'untracked', id: `n${i}`, body: '' }));
    return [
      group('Staged', worktree.staged, 'staged', 's'),
      group('Not staged', worktree.unstaged, 'unstaged', 'u'),
      { title: 'Untracked', files: untracked, note: '' },
    ].filter(g => g.files.length);
  }, keep);
}

function group(title, { files, patch, truncated }, kind, prefix) {
  const { sections, hidden } = patchSections(patch, MAX_LINES);
  const notes = [hidden && `${plural(hidden, 'more line')} not shown here`, truncated && 'the diff is cut at 4 MB'].filter(Boolean);
  return {
    title,
    files: files.map((f, i) => ({ ...f, kind, id: `${prefix}${i}`, body: sections[i] ?? '' })),
    note: notes.length ? `${notes.join(', ')}: open a file in the diff view to see all of it` : '',
  };
}

async function open(source, titleHtml, title, load, keep = false) {
  requested = source.key;
  if (!keep) shown = null;
  $('detail').hidden = $('split').hidden = false;
  $('detail-title').innerHTML = titleHtml;
  $('detail-title').title = title;
  $('open-viewer').disabled = !shown;
  try {
    const groups = await load();
    if (requested !== source.key) return;
    const folds = keep ? keptFolds() : null;
    shown = { source, files: groups.flatMap(g => g.files) };
    fill(groups, folds);
  } catch (e) {
    if (requested !== source.key) return;
    shown = null;
    $('files').innerHTML = '';
    $('files-count').textContent = '';
    $('diff').innerHTML = `<div id="error">${esc(e.message)}</div>`;
  }
}

const fileKey = f => `${f.kind}:${f.path}`;
const fileLabel = f => esc(f.from ? `${f.from} → ${f.path}` : f.path);

function counts(f) {
  if (f.kind === 'untracked') return '<span class="n">new</span>';
  if (f.add === null) return '<span class="n">binary</span>';
  return `<span class="n"><b>+${f.add}</b> <i>−${f.del}</i></span>`;
}

function keptFolds() {
  const closed = new Set([...$('diff').querySelectorAll('.fsec.closed')].map(s => s.dataset.key));
  return { closed, diff: $('diff').scrollTop, files: $('files').scrollTop };
}

function fill(groups, folds) {
  const titled = groups.length > 1 || Boolean(groups[0]?.title);
  const untracked = f => f.kind === 'untracked';
  const section = f => {
    const closed = untracked(f) || folds?.closed.has(fileKey(f));
    return `<div class="fsec${closed ? ' closed' : ''}${untracked(f) ? ' untracked' : ''}" id="${f.id}" data-key="${esc(fileKey(f))}">`
      + `<div class="fh"><span class="st st-${f.status}">${f.status}</span><span class="p">${fileLabel(f)}</span>${counts(f)}`
      + '<button class="open-v" title="Open in the diff view">⤢</button>'
      + `${untracked(f) ? '' : '<button class="fold" title="Fold or unfold this file">▾</button>'}</div><div class="fbody">${f.body}</div></div>`;
  };
  const header = groups[0]?.header ? groups[0].header.replace(/\n$/, '').split('\n').map(l => `<span>${esc(l) || ' '}</span>`).join('') : '';
  $('diff').innerHTML = header + groups.map(g => (titled ? `<div class="grp">${esc(g.title)} · ${g.files.length}</div>` : '')
    + g.files.map(section).join('') + (g.note ? `<span class="mt">${esc(g.note)}</span>` : '')).join('');
  $('files').innerHTML = groups.map(g => (titled ? `<div class="file-grp">${esc(g.title)}</div>` : '')
    + g.files.map(f => `<div class="file" data-id="${f.id}" title="${fileLabel(f)}"><span class="st st-${f.status}">${f.status}</span>`
      + `<span class="p">${fileLabel(f)}</span>${counts(f)}</div>`).join('')).join('')
    || '<div class="file"><span class="p">no file changes</span></div>';
  $('diff').scrollTop = folds?.diff ?? 0;
  $('files').scrollTop = folds?.files ?? 0;
  $('files-count').textContent = plural(shown.files.length, 'file');
  $('open-viewer').disabled = !shown.files.length;
  updateFoldAll();
}

function updateFoldAll() {
  const sections = [...$('diff').querySelectorAll('.fsec:not(.untracked)')];
  $('fold-all').textContent = sections.some(s => !s.classList.contains('closed')) ? 'Collapse all' : 'Expand all';
  $('fold-all').disabled = !sections.length;
}

function openViewer(id) {
  if (!shown) return;
  viewer.open(shown.source, shown.files, Math.max(0, shown.files.findIndex(f => f.id === id)));
}

$('close').addEventListener('click', close);

$('detail-title').addEventListener('click', e => {
  const hash = e.target.closest('.h[data-h]')?.dataset.h;
  if (hash) copy(hash);
});

$('files').addEventListener('click', e => {
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
  const section = e.target.closest('.fsec');
  if (!section) return;
  if (e.target.closest('.open-v')) return openViewer(section.id);
  if (!e.target.closest('.fh') || section.classList.contains('untracked')) return;
  section.classList.toggle('closed');
  updateFoldAll();
});

$('fold-all').addEventListener('click', () => {
  const sections = [...$('diff').querySelectorAll('.fsec:not(.untracked)')];
  const fold = sections.some(s => !s.classList.contains('closed'));
  sections.forEach(s => s.classList.toggle('closed', fold));
  updateFoldAll();
});

$('open-viewer').addEventListener('click', () => openViewer(shown?.files[0]?.id));

$('split').addEventListener('mousedown', e => drag(e, ev => {
  $('detail').style.width = `${Math.max(240, window.innerWidth - ev.clientX)}px`;
  log.render();
}, () => state.store.set('detailWidth', $('detail').offsetWidth)));
