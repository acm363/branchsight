import { $, closeLayer, copy, openLayer } from '../shared/dom.js';
import { SHORT_HASH, esc, plural } from '../shared/format.js';
import { t } from '../shared/i18n.js';
import * as log from '../history/list.js';
import * as panel from '../changes/panel.js';
import { commitAt, state } from '../shared/state.js';
import { checkout, cherryPick, deleteLocal, deleteRemote, drop, merge, pull, push, rebase, reset, resetToRemote, revert } from './commands.js';
import { createBranch, createTag, deleteTag, pushTag } from './edit.js';

const commits = n => plural(n, 'commit');

function syncItems(head, remote) {
  const { name, ahead = 0, behind = 0 } = head, upstream = { name: remote, remote: head.upstreamRemote };
  if (!ahead && !behind) return [];
  if (!ahead) return [[t('menu.syncBringIn', { name, remote, commits: commits(behind) }), () => pull(name, remote)]];
  if (!behind) return [[t('menu.pushSend', { name, remote, commits: commits(ahead) }), () => push(name, upstream)]];
  return [
    [t('menu.diverged', { name, remote, ahead: commits(ahead), behind: commits(behind) }), null, 'note'],
    [t('menu.syncMergeRebase', { name, remote }), () => pull(name, remote)],
    [t('menu.resetToRemote', { name, remote, commits: commits(ahead) }), () => resetToRemote(name, remote, ahead), 'danger'],
    [t('menu.pushForceLease', { name, remote }), () => push(name, upstream, 'force-with-lease')],
  ];
}

const tracks = (head, remote) => head?.upstream === remote && head.upstream === `${head.upstreamRemote}/${head.name}`;

const menuLayer = { contains: target => $('ctx').contains(target), close: () => { $('ctx').hidden = true; } };

function historyOfHead() {
  const byHash = new Map(state.entries.filter(e => e.h).map(e => [e.h, e]));
  const reached = new Set(), pending = [state.repo.head];
  while (pending.length) {
    const hash = pending.pop();
    if (reached.has(hash) || !byHash.has(hash)) continue;
    reached.add(hash);
    pending.push(...byHash.get(hash).p);
  }
  const complete = byHash.has(state.repo.head) && state.entries.length < +$('limit').value;
  return { mine: hash => reached.has(hash), foreign: hash => complete && !reached.has(hash) };
}

function menuItems(i, commit, label) {
  const items = [], history = historyOfHead();
  const into = state.repo.branch || 'HEAD';
  const mergeItem = (name, shown = name) => [t('menu.merge', { shown, into }), () => merge(name, shown)];
  const rebaseItem = (name, shown = name) => [t('menu.rebase', { into, shown }), () => rebase(name, shown)];
  const base = commitAt(state.cursor);
  if (base && state.cursor !== i) items.push([t('menu.compareSelected', { hash: base.h.slice(0, SHORT_HASH) }), () => panel.showCompare(state.cursor, i)], null);
  if (label?.kind === 'head') {
    items.push([t('menu.checkout', { name: label.name }), () => checkout(label.name)]);
    if (!label.current && !history.mine(commit.h)) items.push(mergeItem(label.name), rebaseItem(label.name));
    const head = state.refs.find(r => r.kind === 'head' && r.name === label.name);
    const upstream = label.upstream && { name: label.upstream, remote: label.upstreamRemote };
    const tracked = upstream && state.refs.some(r => r.kind === 'remote' && r.name === upstream.name);
    if (tracked && tracks(head, upstream.name)) items.push(...syncItems(head, upstream.name));
    else {
      if (tracked && head?.behind) items.push([t('menu.syncWith', { name: label.name, remote: upstream.name }), () => pull(label.name, upstream.name)]);
      if (state.repo.remotes.length) items.push([upstream ? t('menu.push', { name: label.name }) : t('menu.pushToRemote', { name: label.name }), () => push(label.name, upstream)]);
    }
    items.push([t('menu.copyBranch'), () => copy(label.name)]);
    if (!label.current) items.push([t('menu.deleteBranch', { name: label.name }), () => deleteLocal(label.name), 'danger']);
  }
  if (label?.kind === 'remote') {
    const [remote] = label.remotes;
    const head = remote.branch && state.refs.find(r => r.kind === 'head' && r.name === remote.branch);
    items.push([t('menu.checkout', { name: remote.name }), () => checkout(remote.name)]);
    if (tracks(head, remote.name)) items.push(...syncItems(head, remote.name));
    else if (head) items.push([t('menu.syncWith', { name: remote.branch, remote: remote.name }), () => pull(remote.branch, remote.name)]);
    if (!history.mine(commit.h)) items.push(mergeItem(remote.name), rebaseItem(remote.name));
    items.push([t('menu.copyBranch'), () => copy(remote.branch ?? remote.name)]);
  }
  label?.remotes.forEach(r => items.push([t('menu.deleteRemote', { name: r.name }), () => deleteRemote(r.name), 'danger']));
  if (label?.kind === 'tag') {
    items.push([t('menu.checkoutTag', { name: label.name }), () => checkout(label.name)]);
    if (!history.mine(commit.h)) items.push(mergeItem(label.name));
    if (state.repo.remotes.length) items.push([t('menu.pushTag', { name: label.name }), () => pushTag(label.name)]);
    items.push([t('menu.copyTag'), () => copy(label.name)], [t('menu.deleteTag', { name: label.name }), () => deleteTag(label.name), 'danger']);
  }
  if (items.length && items.at(-1) !== null) items.push(null);
  const short = commit.h.slice(0, SHORT_HASH), here = commit.h === state.repo.head;
  items.push([t('menu.checkoutCommit', { short }), () => checkout(commit.h, short)]);
  items.push([t('menu.createBranchHere'), () => createBranch(commit)], [t('menu.createTagHere'), () => createTag(commit)]);
  if (!history.mine(commit.h)) items.push([t('menu.cherryPick'), () => cherryPick(commit)]);
  if (!history.foreign(commit.h)) items.push([t('menu.revert'), () => revert(commit)], [t('menu.drop'), () => drop(commit), 'danger']);
  items.push(null);
  if (!here && !history.mine(commit.h)) items.push(mergeItem(commit.h, t('menu.commitShort', { short })), rebaseItem(commit.h, t('menu.commitShort', { short })));
  items.push([t('menu.resetHere', { into }), () => reset(commit)], null);
  items.push([t('menu.copyHash'), () => copy(commit.h)]);
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
  const html = (item, k) => {
    if (!item) return '<hr>';
    if (!item[1]) return `<div class="note">${esc(item[0])}</div>`;
    return `<button data-i="${k}"${item[2] ? ` class="${item[2]}"` : ''}>${esc(item[0])}</button>`;
  };
  menu.innerHTML = items.map(html).join('');
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

export function openColumnMenu(e) {
  e.preventDefault();
  const menu = $('ctx');
  const render = () => {
    menu.innerHTML = log.columnMenuItems()
      .map(([label, name, on]) => `<button data-col="${name}" class="check${on ? ' on' : ''}">${esc(label)}</button>`)
      .join('');
  };
  render();
  menu.onclick = ev => {
    const button = ev.target.closest('button[data-col]');
    if (!button) return;
    log.toggleColumn(button.dataset.col);
    render();
  };
  menu.hidden = false;
  openLayer(menuLayer);
  const { width, height } = menu.getBoundingClientRect();
  menu.style.left = `${Math.min(e.clientX, window.innerWidth - width - 4)}px`;
  menu.style.top = `${Math.min(e.clientY, window.innerHeight - height - 4)}px`;
}

$('cols').addEventListener('contextmenu', openColumnMenu);
$('list').addEventListener('scroll', () => closeLayer(menuLayer), { passive: true });
