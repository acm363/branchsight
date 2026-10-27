import { post } from './api.js';
import { setSelection } from './branches.js';
import { $, closeLayer, confirmAction, copy, openLayer, toast } from './dom.js';
import { esc } from './format.js';
import { poll, reload } from './history.js';
import * as log from './log.js';
import * as panel from './panel.js';
import { commitAt, state } from './state.js';

const AUTO_FETCH_MS = 120000, AUTO_FETCH_CHECK_MS = 10000;

let fetching = false, lastFetch = 0;

async function run(endpoint, name, busy, done) {
  toast(busy, 60000);
  try {
    const { out } = await post(endpoint, { ref: name });
    toast(out.trim() || done, 3000);
  } catch (e) {
    toast(e.message.split('\n')[0], 7000);
  }
  await reload().catch(() => {});
}

async function checkout(name, label) {
  await run('/api/checkout', name, `checking out ${label}…`, `checked out ${label}`);
  const { head } = state.repo;
  if (head && !state.entries.some(e => e.h === head) && !state.selected.includes('HEAD')) await setSelection([...state.selected, 'HEAD']);
  log.focus(head);
}

async function deleteRemote(name) {
  const ok = await confirmAction(`Delete ${name} on the remote? Everyone who uses it loses the branch; BranchSight cannot undo it.`, 'Delete');
  if (ok) await run('/api/delete-remote', name, `deleting ${name}…`, `deleted ${name}`);
}

export async function fetchRemotes() {
  if (fetching || !state.repo.path) return;
  fetching = true;
  const button = $('fetch');
  button.disabled = true;
  button.textContent = '⇣ fetching…';
  try {
    await post('/api/fetch');
    button.title = `Fetch all remotes (f) · last ${new Date().toLocaleTimeString()}`;
    button.classList.remove('err');
  } catch (e) {
    button.title = `Fetch failed: ${e.message}`;
    button.classList.add('err');
    toast(`fetch failed: ${e.message.split('\n')[0]}`, 5000);
  } finally {
    lastFetch = Date.now();
    fetching = false;
    button.disabled = false;
    button.textContent = '⇣ fetch';
    poll();
  }
}

function autoFetch() {
  if ($('auto-fetch').checked && !document.hidden && Date.now() - lastFetch >= AUTO_FETCH_MS) fetchRemotes();
}

export function startAutoFetch() {
  autoFetch();
  setInterval(autoFetch, AUTO_FETCH_CHECK_MS);
  document.addEventListener('visibilitychange', autoFetch);
}

const menuLayer = { contains: target => $('ctx').contains(target), close: () => { $('ctx').hidden = true; } };

function menuItems(i, commit, label) {
  const items = [];
  const base = commitAt(state.cursor);
  if (base && state.cursor !== i) items.push([`Compare with selected ${base.h.slice(0, 7)}`, () => panel.showCompare(state.cursor, i)], null);
  if (label?.kind === 'head') {
    items.push([`Checkout ${label.name}`, () => checkout(label.name, label.name)]);
    items.push(['Copy branch name', () => copy(label.name)]);
  }
  if (label?.kind === 'remote') {
    const [remote] = label.remotes;
    items.push([`Checkout ${remote.name}`, () => checkout(remote.name, remote.name)]);
    items.push(['Copy branch name', () => copy(remote.branch ?? remote.name)]);
  }
  label?.remotes.forEach(r => items.push([`Delete remote branch ${r.name}`, () => deleteRemote(r.name), 'danger']));
  if (label?.kind === 'tag') {
    items.push([`Checkout tag ${label.name} (detached)`, () => checkout(label.name, label.name)]);
    items.push(['Copy tag name', () => copy(label.name)]);
  }
  if (items.length && items.at(-1) !== null) items.push(null);
  items.push([`Checkout commit ${commit.h.slice(0, 7)} (detached)`, () => checkout(commit.h, commit.h.slice(0, 7))]);
  items.push(['Copy commit hash', () => copy(commit.h)]);
  return items;
}

export function openContextMenu(e) {
  const row = e.target.closest('.row');
  if (!row) return;
  e.preventDefault();
  const i = +row.dataset.i, commit = commitAt(i);
  if (!commit) return;
  const clicked = e.target.closest('.ref')?.dataset;
  const label = clicked && log.labelsOf(commit).find(l => l.kind === clicked.kind && l.name === clicked.name);
  const items = menuItems(i, commit, label);
  const menu = $('ctx');
  menu.innerHTML = items.map((item, k) => item ? `<button data-i="${k}"${item[2] ? ` class="${item[2]}"` : ''}>${esc(item[0])}</button>` : '<hr>').join('');
  menu.onclick = ev => {
    const button = ev.target.closest('button[data-i]');
    if (!button) return;
    closeLayer(menuLayer);
    items[+button.dataset.i][1]();
  };
  menu.hidden = false;
  openLayer(menuLayer);
  const { width, height } = menu.getBoundingClientRect();
  menu.style.left = `${Math.min(e.clientX, window.innerWidth - width - 4)}px`;
  menu.style.top = `${Math.min(e.clientY, window.innerHeight - height - 4)}px`;
}

$('list').addEventListener('scroll', () => closeLayer(menuLayer), { passive: true });
$('fetch').addEventListener('click', fetchRemotes);
$('auto-fetch').addEventListener('change', e => {
  state.store.set('autoFetch', e.target.checked);
  autoFetch();
});
