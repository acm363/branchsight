import { post } from '../shared/api.js';
import { setSelection } from '../header/branches.js';
import { $, ask, confirmAction, toast } from '../shared/dom.js';
import { SHORT_HASH, esc, plural } from '../shared/format.js';
import * as panel from '../changes/panel.js';
import * as log from '../history/list.js';
import { commitAt, isWorktree, state } from '../shared/state.js';
import { poll, reload } from '../history/sync.js';

const enterOnlyOnButtons = e => e.key === 'Enter' && !e.target.closest('button') && e.preventDefault();
const AUTO_FETCH_MS = 120000, AUTO_FETCH_CHECK_MS = 10000, ACTION_MS = 15 * 60 * 1000;

let fetching = false, lastFetch = 0;

const headCommit = () => state.repo.head;
const tipOf = branch => () => state.refs.find(r => r.kind === 'head' && r.name === branch)?.hash;

async function run(endpoint, params, busy, focus = headCommit) {
  toast(busy, ACTION_MS);
  let done = false, stopped = false;
  try {
    const { out } = await post(endpoint, params);
    toast(out, 3000);
    done = true;
  } catch (e) {
    toast(e.message.split('\n').slice(0, 3).join('\n'), 9000);
    stopped = e.message.includes('stopped on conflicts') || e.message.includes('conflict markers');
  }
  await reload().catch(() => {});
  if (stopped) focusUncommitted();
  else if (done && focus) await focusOn(focus());
}

function focusUncommitted() {
  const i = state.entries.findIndex(isWorktree);
  if (i < 0) return;
  log.setCursor(i, { scroll: 'center' });
  if (panel.isOpen()) panel.showSoon(state.entries[i], 0);
}

export const finishOperation = operation => run('/api/continue', {}, `finishing the ${operation}…`);

export const abortOperation = operation => run('/api/abort', {}, `aborting the ${operation}…`);

async function focusOn(hash) {
  if (!hash) return;
  const hidden = !state.entries.some(e => e.h === hash);
  if (hidden && hash === state.repo.head && !state.selected.includes('HEAD')) await setSelection([...state.selected, 'HEAD']);
  log.focus(hash);
  if (panel.isOpen() && commitAt(state.cursor)?.h === hash) panel.showSoon(state.entries[state.cursor], 0);
}

export async function checkout(name, label = name) {
  await run('/api/checkout', { ref: name }, `checking out ${label}…`);
}

export async function deleteRemote(name) {
  const ok = await confirmAction(`Delete ${name} on the remote? Everyone who uses it loses the branch; BranchSight cannot undo it.`, 'Delete');
  if (ok) await run('/api/delete-remote', { ref: name }, `deleting ${name}…`, null);
}

export async function push(branch, upstream, initialMode = 'normal') {
  const { remotes } = state.repo;
  const dialog = $('push'), select = $('push-remote');
  $('push-branch').textContent = branch;
  select.innerHTML = remotes.map(r => `<option>${esc(r)}</option>`).join('');
  select.value = [upstream?.remote, 'origin'].find(r => remotes.includes(r)) ?? remotes[0];
  const note = () => {
    const target = `${select.value}/${branch}`;
    $('push-note').hidden = !upstream || upstream.name === target;
    $('push-note').textContent = `${branch} tracks ${upstream?.name} today; after this push it tracks ${target}.`;
  };
  note();
  select.onchange = note;
  dialog.onkeydown = enterOnlyOnButtons;
  dialog.querySelector(`[name="mode"][value="${initialMode}"]`).checked = true;
  if ((await ask(dialog)) !== 'ok') return;
  const remote = select.value, mode = dialog.querySelector('[name="mode"]:checked').value;
  const lost = `Commits on ${remote}/${branch} that you do not have are lost for everyone who uses it.`;
  if (mode === 'force' && !(await confirmAction(`Force-push ${branch} to ${remote}? ${lost}`, 'Force push'))) return;
  await run('/api/push', { ref: branch, remote, mode }, `pushing ${branch} to ${remote}…`, null);
}

const currentBranch = () => state.repo.branch || 'HEAD';

export async function resetToRemote(branch, remote, own) {
  const text = `Reset ${branch} to ${remote}? It drops the ${plural(own, 'commit')} that ${remote} does not have; git reflog can still bring them back. Uncommitted changes are kept, or git refuses.`;
  if (await confirmAction(text, 'Reset')) await run('/api/reset-to-remote', { branch, from: remote }, `resetting ${branch} to ${remote}…`, tipOf(branch));
}

export async function pull(branch, from) {
  const dialog = $('pull'), elsewhere = branch !== state.repo.branch;
  $('pull-from').textContent = from;
  $('pull-branch').textContent = branch;
  $('pull-note').hidden = !elsewhere;
  $('pull-note').textContent = `${branch} is not checked out: if it only lags behind it is brought up where it is, otherwise BranchSight switches to it first.`;
  dialog.querySelector('[name="mode"][value="merge"]').checked = true;
  dialog.onkeydown = enterOnlyOnButtons;
  if ((await ask(dialog)) !== 'ok') return;
  const mode = dialog.querySelector('[name="mode"]:checked').value;
  await run('/api/pull', { branch, from, mode }, `syncing ${branch} with ${from}…`, tipOf(branch));
}

export async function merge(name, shown = name) {
  const dialog = $('merge'), noCommit = $('merge-no-commit');
  const into = currentBranch();
  const mode = () => dialog.querySelector('[name="mode"]:checked').value;
  $('merge-source').textContent = shown;
  $('merge-target').textContent = into;
  dialog.querySelector('[name="mode"][value="fast-forward"]').checked = true;
  noCommit.checked = noCommit.disabled = false;
  dialog.onchange = () => { noCommit.disabled = mode() === 'squash'; };
  dialog.onkeydown = enterOnlyOnButtons;
  if ((await ask(dialog)) !== 'ok') return;
  await run('/api/merge', { ref: name, mode: mode(), commit: noCommit.checked ? '0' : '1' }, `merging ${shown} into ${into}…`);
}

async function apply(endpoint, verb, commit) {
  const dialog = $('apply'), noCommit = $('apply-no-commit'), short = commit.h.slice(0, SHORT_HASH);
  $('apply-text').textContent = `${verb} ${short} "${commit.s}" on ${currentBranch()}`;
  $('apply-ok').textContent = verb;
  noCommit.checked = false;
  dialog.onkeydown = enterOnlyOnButtons;
  if ((await ask(dialog)) !== 'ok') return;
  await run(endpoint, { h: commit.h, commit: noCommit.checked ? '0' : '1' }, `${verb.toLowerCase()} ${short}…`);
}

export const cherryPick = commit => apply('/api/cherry-pick', 'Cherry pick', commit);

export const revert = commit => apply('/api/revert', 'Revert', commit);

export async function drop(commit) {
  const short = commit.h.slice(0, SHORT_HASH), into = currentBranch();
  const text = `Drop ${short} "${commit.s}" from ${into}? Its changes leave the branch and every commit after it is rewritten; if they were pushed, the branch will need a force push.`;
  if (await confirmAction(text, 'Drop')) await run('/api/drop', { h: commit.h }, `dropping ${short}…`);
}

export async function rebase(name, shown = name) {
  const into = currentBranch();
  const text = `Rebase ${into} on ${shown}? Its own commits are replayed on top of ${shown} and get new hashes; if they were pushed, the branch will need a force push.`;
  if (await confirmAction(text, 'Rebase')) await run('/api/rebase', { ref: name }, `rebasing ${into} on ${shown}…`);
}

export async function reset(commit) {
  const dialog = $('reset'), short = commit.h.slice(0, SHORT_HASH), into = currentBranch();
  $('reset-branch').textContent = into;
  $('reset-commit').textContent = `${short} "${commit.s}"`;
  dialog.querySelector('[name="mode"][value="mixed"]').checked = true;
  dialog.onkeydown = enterOnlyOnButtons;
  if ((await ask(dialog)) !== 'ok') return;
  const mode = dialog.querySelector('[name="mode"]:checked').value;
  const lost = `Every commit after ${short} and every uncommitted change are discarded; git cannot bring back uncommitted work.`;
  if (mode === 'hard' && !(await confirmAction(`Hard reset ${into} to ${short}? ${lost}`, 'Discard and reset'))) return;
  await run('/api/reset', { h: commit.h, mode }, `resetting ${into} to ${short}…`);
}

export async function fetchRemotes({ unattended = false } = {}) {
  if (fetching || !state.repo.path) return;
  fetching = true;
  const button = $('fetch');
  button.disabled = true;
  button.textContent = '⇣ fetching…';
  try {
    await post('/api/fetch', unattended ? { auto: '1' } : {});
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
  if ($('auto-fetch').checked && !document.hidden && Date.now() - lastFetch >= AUTO_FETCH_MS) fetchRemotes({ unattended: true });
}

export function startAutoFetch() {
  autoFetch();
  setInterval(autoFetch, AUTO_FETCH_CHECK_MS);
  document.addEventListener('visibilitychange', autoFetch);
}

$('fetch').addEventListener('click', () => fetchRemotes());
$('auto-fetch').addEventListener('change', e => {
  state.store.set('autoFetch', e.target.checked);
  autoFetch();
});
