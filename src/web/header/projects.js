import { getJson, inDesktopApp, pickFolder } from '../shared/api.js';
import { $ } from '../shared/dom.js';
import { color, esc, fuzzy, highlight, pathKey, samePath, shortPath } from '../shared/format.js';
import { picker } from '../shared/picker.js';
import { prefs } from '../shared/prefs.js';
import { state } from '../shared/state.js';

const PATH_LIKE = /^(\/|~|[a-zA-Z]:[\\/])/;
const MAX_RECENT = 30;

let known = null;
let configRoots = [];

const dedup = list => [...new Map(list.map(p => [pathKey(p), p])).values()];
const customRoots = () => prefs.get('roots') || [];

export function useRoots(roots) {
  configRoots = roots;
  state.roots = dedup([...configRoots, ...customRoots()]);
}

const menu = picker({
  anchor: $('projects'),
  panel: $('pmenu'),
  input: $('pfilter'),
  list: $('plist'),
  modal: () => !state.repo.path,
  items: matching,
  row: ({ project, match, action }, i, current) => {
    const cur = current ? ' cur' : '';
    if (action === 'scan') {
      return `<div class="pick scan${cur}" data-i="${i}"><span class="name">Scan <b>${esc(project.path)}</b> for repositories</span></div>`;
    }
    if (action === 'open') {
      return `<div class="pick open${cur}" data-i="${i}"><span class="name">Open <b>${esc(project.path)}</b></span></div>`;
    }
    const short = shortPath(project.path, state.roots);
    return `<div class="pick${samePath(project.path, state.repo.path) ? ' on' : ''}${cur}" data-i="${i}">`
      + `<span class="name">${highlight(project.name, match.marks)}</span><span class="when">${short === project.name ? '' : esc(short)}</span></div>`;
  },
  choose: ({ project, action }) => (action === 'scan' ? scanFolder(project.path) : switchTo(project.path)),
  empty: () => (known ? 'no git repository under the roots' : 'looking for repositories…'),
});

const fetchProjects = () => getJson('/api/projects', customRoots().map(root => ['root', root]));

export async function list() {
  known ??= await fetchProjects();
  return known;
}

function matching(query) {
  const recent = (prefs.get('recent') || []).map(pathKey);
  const rank = project => {
    const i = recent.indexOf(pathKey(project.path));
    return i < 0 ? recent.length : i;
  };
  const byPath = project => /[\\/]/.test(query) && fuzzy(query, shortPath(project.path, state.roots)) ? { score: 500, marks: [] } : null;
  const found = (known || [])
    .map((project, order) => ({ project, order, match: fuzzy(query, project.name) || byPath(project) }))
    .filter(x => x.match)
    .sort(query ? (a, b) => a.match.score - b.match.score || a.order - b.order : (a, b) => rank(a.project) - rank(b.project) || a.order - b.order);
  if (PATH_LIKE.test(query)) {
    const synthetic = action => ({ project: { name: query, path: query }, action, match: { marks: [] } });
    found.unshift(synthetic('open'), synthetic('scan'));
  }
  return found;
}

export async function open() {
  menu.open();
  renderRoots();
  await refresh();
}

async function refresh() {
  try {
    known = await fetchProjects();
  } catch (e) {
    note(e.message);
  }
  if (menu.isOpen) menu.render();
}

function switchTo(path) {
  if (samePath(path, state.repo.path)) return menu.close();
  location.href = `${location.pathname}?repo=${encodeURIComponent(path)}`;
}

async function scanFolder(path) {
  addRoot(path);
  note('');
  await refresh();
  if (known && !known.some(p => pathKey(p.path).startsWith(`${pathKey(path)}/`) || samePath(p.path, path))) {
    note(`no git repository under ${path}`);
  }
}

function addRoot(path) {
  if (!customRoots().some(r => samePath(r, path))) saveRoots([...customRoots(), path]);
}

function removeRoot(path) {
  saveRoots(customRoots().filter(r => !samePath(r, path)));
}

function saveRoots(roots) {
  prefs.set('roots', roots);
  useRoots(configRoots);
  renderRoots();
}

function renderRoots() {
  const custom = customRoots();
  const line = (path, removable) => `<div class="root">${removable ? `<button class="rx" data-root="${esc(path)}" title="Remove">×</button>` : '<span class="rx"></span>'}<span class="rp">${esc(path)}</span></div>`;
  const add = inDesktopApp ? '<button id="add-folder" class="rbtn">Add folder…</button>' : '<div class="rhint">Type a path above to scan a folder</div>';
  $('proots').innerHTML = '<div class="rhead">Folders to scan</div>'
    + configRoots.map(root => line(root, false)).join('')
    + custom.map(root => line(root, true)).join('')
    + add;
}

export function setCurrent(repo) {
  prefs.set('last', repo.path);
  prefs.set('recent', [repo.path, ...(prefs.get('recent') || []).filter(p => !samePath(p, repo.path))].slice(0, MAX_RECENT));
  document.title = `${repo.name} · BranchSight`;
  $('repo').textContent = repo.name;
  $('proj-chip').title = `${repo.path} — switch project (p)`;
  $('avatar').textContent = (repo.name[0] || '?').toUpperCase();
  $('avatar').style.background = color([...repo.name].reduce((h, ch) => (h * 31 + ch.charCodeAt(0)) >>> 0, 0));
}

export function closeProject() {
  prefs.set('last', null);
  location.href = `${location.pathname}?start`;
}

function note(text) {
  $('pnote').textContent = text;
  $('pnote').hidden = !text;
}

export function showStart(problem = '') {
  document.body.classList.add('start');
  document.title = 'BranchSight';
  $('repo').textContent = 'no project';
  note(problem);
  return open();
}

$('proj-chip').addEventListener('click', () => (menu.isOpen ? menu.close() : open()));

$('proots').addEventListener('click', async e => {
  const remove = e.target.closest('.rx[data-root]');
  if (remove) {
    removeRoot(remove.dataset.root);
    return refresh();
  }
  if (e.target.closest('#add-folder')) {
    const path = await pickFolder();
    if (path) await scanFolder(path);
  }
});
