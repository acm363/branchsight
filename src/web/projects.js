import { getJson } from './api.js';
import { $ } from './dom.js';
import { esc, fuzzy, highlight, pathKey, samePath, shortPath } from './format.js';
import { picker } from './picker.js';
import { prefs } from './prefs.js';
import { state } from './state.js';

const PATH_LIKE = /^(\/|~|[a-zA-Z]:[\\/])/;

let known = null;

const menu = picker({
  anchor: $('projects'),
  panel: $('pmenu'),
  input: $('pfilter'),
  list: $('plist'),
  modal: () => !state.repo.path,
  items: matching,
  row: ({ project, match }, i, current) => {
    const short = shortPath(project.path, state.roots);
    return `<div class="pick${samePath(project.path, state.repo.path) ? ' on' : ''}${current ? ' cur' : ''}" data-i="${i}">`
      + `<span class="name">${highlight(project.name, match.marks)}</span><span class="when">${short === project.name ? '' : esc(short)}</span></div>`;
  },
  choose: ({ project }) => switchTo(project.path),
  empty: () => (known ? 'no git repository under the roots' : 'looking for repositories…'),
});

export async function list() {
  known ??= await getJson('/api/projects');
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
  if (PATH_LIKE.test(query)) found.unshift({ project: { name: `open ${query}`, path: query }, match: { marks: [] } });
  return found;
}

export async function open() {
  menu.open();
  try {
    known = await getJson('/api/projects');
  } catch (e) {
    note(e.message);
  }
  if (menu.isOpen) menu.render();
}

function switchTo(path) {
  if (samePath(path, state.repo.path)) return menu.close();
  location.href = `${location.pathname}?repo=${encodeURIComponent(path)}`;
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
