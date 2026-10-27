import { getJson } from '../shared/api.js';
import { $, copy, drag, toast } from '../shared/dom.js';
import { SHORT_HASH, esc, fileLabel, plural, statusClass } from '../shared/format.js';
import { refName } from '../history/labels.js';
import * as log from '../history/list.js';
import { lineHtml, patchHtml } from './patch.js';
import { commitAt, isWorktree, state } from '../shared/state.js';
import { onLog } from '../history/sync.js';
import * as viewer from './viewer.js';

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
  $('detail').hidden = $('split').hidden = true;
  requestAnimationFrame(log.render);
}

function clearCompare() {
  if (!state.compareHash) return;
  state.compareHash = null;
  log.render();
}

const hashSpan = (hash, length = 10) => `<span class="h" data-h="${hash}" title="Copy ${hash}">${hash.slice(0, length)}</span>`;

function sideLabel(hash) {
  const commit = state.entries.find(e => e.h === hash);
  const name = refName(state.refsByHash.get(hash));
  return `${name ? `${name} · ` : ''}${hash.slice(0, SHORT_HASH)}${commit ? ` ${commit.s}` : ''}`;
}

function showCommit(commit) {
  clearCompare();
  const parent = commit.p[0];
  const sides = [parent ? `parent · ${sideLabel(parent)}` : 'nothing (first commit)', sideLabel(commit.h)];
  const source = { kind: 'commit', key: commit.h, h: commit.h };
  return open(source, `${hashSpan(commit.h)}${esc(commit.s)}`, commit.s, async budget => {
    const changes = await getJson('/api/commit', { h: commit.h });
    return [{ header: changes.header, ...group('', changes, 'commit', sides, budget) }];
  });
}

export function showCompare(i, j) {
  const a = commitAt(i), b = commitAt(j);
  if (!a || !b) return toast('compare two commits, not the uncommitted changes', 3000);
  const [newer, older] = i < j ? [a, b] : [b, a];
  state.compareHash = b.h;
  log.render();
  const source = { kind: 'compare', key: `${older.h}..${newer.h}`, from: older.h, to: newer.h };
  const title = `<span class="cmp-label">Compare</span>${hashSpan(older.h, SHORT_HASH)}<span class="arrow">→</span>${hashSpan(newer.h, SHORT_HASH)}`;
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
  return open(source, '<span class="wt-title">Uncommitted changes</span>', 'Uncommitted changes', async budget => {
    const worktree = await getJson('/api/worktree');
    const untracked = worktree.untracked.map((path, i) => ({
      status: '?', path, add: null, del: null, kind: 'untracked', id: `untracked-${i}`, body: '', sides: ['nothing (new file)', 'Working tree'],
    }));
    return [
      group('Staged', worktree.staged, 'staged', [head, 'Staged (index)'], budget),
      group('Not staged', worktree.unstaged, 'unstaged', ['Staged (index)', 'Working tree'], budget),
      { title: 'Untracked', files: untracked, note: '' },
    ].filter(g => g.files.length);
  }, keep);
}

function group(title, { files, truncated }, kind, sides, budget) {
  let hidden = 0;
  const listed = files.map(({ patch, ...file }, i) => {
    const rendered = patchHtml(patch, budget.lines);
    budget.lines -= rendered.shown;
    hidden += rendered.hidden;
    return { ...file, kind, sides, id: `${kind}-${i}`, body: rendered.html };
  });
  const notes = [hidden && `${plural(hidden, 'more line')} not shown here`, truncated && 'the diff is cut at 4 MB'].filter(Boolean);
  return { title, files: listed, note: notes.length ? `${notes.join(', ')}: open a file in the diff view to see all of it` : '' };
}

async function open(source, titleHtml, title, load, keep = false) {
  requested = source.key;
  if (!keep) shown = null;
  $('detail').hidden = $('split').hidden = false;
  $('detail-title').innerHTML = titleHtml;
  $('detail-title').title = title;
  $('open-viewer').disabled = !shown;
  try {
    const groups = await load({ lines: MAX_LINES });
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

function counts(f) {
  if (f.kind === 'untracked') return '<span class="n">new</span>';
  if (f.status === 'U') return '<span class="n">conflict</span>';
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
  const label = f => esc(fileLabel(f));
  const section = f => {
    const closed = untracked(f) || folds?.closed.has(fileKey(f));
    return `<div class="fsec${closed ? ' closed' : ''}${untracked(f) ? ' untracked' : ''}" id="${f.id}" data-key="${esc(fileKey(f))}">`
      + `<div class="fh"><span class="st ${statusClass(f)}">${f.status}</span><span class="p">${label(f)}</span>${counts(f)}`
      + '<button class="open-v" title="Open in the diff view">⤢</button>'
      + `${untracked(f) ? '' : '<button class="fold" title="Fold or unfold this file">▾</button>'}</div><div class="fbody">${f.body}</div></div>`;
  };
  const header = groups[0]?.header?.replace(/\n$/, '').split('\n').map(line => lineHtml(line)).join('') ?? '';
  $('diff').innerHTML = header + groups.map(g => (titled ? `<div class="grp">${esc(g.title)} · ${g.files.length}</div>` : '')
    + g.files.map(section).join('') + (g.note ? lineHtml(g.note, 'mt') : '')).join('');
  $('files').innerHTML = groups.map(g => (titled ? `<div class="file-grp">${esc(g.title)}</div>` : '')
    + g.files.map(f => `<div class="file" data-id="${f.id}" title="${label(f)}"><span class="st ${statusClass(f)}">${f.status}</span>`
      + `<span class="p">${label(f)}</span>${counts(f)}</div>`).join('')).join('')
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

onLog(worktree => {
  if (requested !== 'worktree' || worktree === drawnWorktree) return;
  drawnWorktree = worktree;
  if (isWorktree(state.entries[0])) showWorktree({ keep: true });
  else close();
});

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
}, () => state.store.set('detailWidth', $('detail').offsetWidth)));
