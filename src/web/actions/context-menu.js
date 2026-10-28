import { $, closeLayer, copy, openLayer } from '../shared/dom.js';
import { SHORT_HASH, esc } from '../shared/format.js';
import * as log from '../history/list.js';
import * as panel from '../changes/panel.js';
import { commitAt, state } from '../shared/state.js';
import { checkout, cherryPick, deleteRemote, drop, merge, pull, push, rebase, reset, revert } from './commands.js';

const menuLayer = { contains: target => $('ctx').contains(target), close: () => { $('ctx').hidden = true; } };

function menuItems(i, commit, label) {
  const items = [];
  const into = state.repo.branch || 'HEAD';
  const mergeItem = (name, shown = name) => [`Merge ${shown} into ${into}…`, () => merge(name, shown)];
  const rebaseItem = (name, shown = name) => [`Rebase ${into} on ${shown}…`, () => rebase(name, shown)];
  const base = commitAt(state.cursor);
  if (base && state.cursor !== i) items.push([`Compare with selected ${base.h.slice(0, SHORT_HASH)}`, () => panel.showCompare(state.cursor, i)], null);
  if (label?.kind === 'head') {
    items.push([`Checkout ${label.name}`, () => checkout(label.name)]);
    if (!label.current) items.push(mergeItem(label.name), rebaseItem(label.name));
    const upstream = label.upstream && { name: label.upstream, remote: label.upstreamRemote };
    const pushed = label.remotes.some(r => !upstream || r.name === upstream.name);
    const tracked = upstream && state.refs.some(r => r.kind === 'remote' && r.name === upstream.name);
    if (tracked && !pushed) items.push([`Pull ${upstream.name} into ${label.name}…`, () => pull(label.name, upstream.name)]);
    if (state.repo.remotes.length && !pushed) items.push([upstream ? `Push ${label.name}…` : `Push ${label.name} to a remote…`, () => push(label.name, upstream)]);
    items.push(['Copy branch name', () => copy(label.name)]);
  }
  if (label?.kind === 'remote') {
    const [remote] = label.remotes;
    const local = remote.branch && state.refs.some(r => r.kind === 'head' && r.name === remote.branch);
    items.push([`Checkout ${remote.name}`, () => checkout(remote.name)]);
    if (local) items.push([`Pull ${remote.name} into ${remote.branch}…`, () => pull(remote.branch, remote.name)]);
    items.push(mergeItem(remote.name), rebaseItem(remote.name));
    items.push(['Copy branch name', () => copy(remote.branch ?? remote.name)]);
  }
  label?.remotes.forEach(r => items.push([`Delete remote branch ${r.name}`, () => deleteRemote(r.name), 'danger']));
  if (label?.kind === 'tag') {
    items.push([`Checkout tag ${label.name} (detached)`, () => checkout(label.name)]);
    items.push(mergeItem(label.name));
    items.push(['Copy tag name', () => copy(label.name)]);
  }
  if (items.length && items.at(-1) !== null) items.push(null);
  const short = commit.h.slice(0, SHORT_HASH), here = commit.h === state.repo.head;
  items.push([`Checkout commit ${short} (detached)`, () => checkout(commit.h, short)]);
  if (!here) items.push(['Cherry pick…', () => cherryPick(commit)]);
  items.push(['Revert…', () => revert(commit)], ['Drop…', () => drop(commit), 'danger'], null);
  if (!here) items.push(mergeItem(commit.h, `commit ${short}`), rebaseItem(commit.h, `commit ${short}`));
  items.push([`Reset ${into} to this commit…`, () => reset(commit)], null);
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
