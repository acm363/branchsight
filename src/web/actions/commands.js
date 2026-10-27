import { post } from '../shared/api.js';
import { setSelection } from '../header/branches.js';
import { $, ask, confirmAction, toast } from '../shared/dom.js';
import { SHORT_HASH, esc } from '../shared/format.js';
import * as log from '../history/list.js';
import { state } from '../shared/state.js';
import { poll, reload } from '../history/sync.js';

const enterOnlyOnButtons = e => e.key === 'Enter' && !e.target.closest('button') && e.preventDefault();
const AUTO_FETCH_MS = 120000, AUTO_FETCH_CHECK_MS = 10000, ACTION_MS = 15 * 60 * 1000;

let fetching = false, lastFetch = 0;

async function run(endpoint, params, busy) {
  toast(busy, ACTION_MS);
  try {
    const { out } = await post(endpoint, params);
    toast(out, 3000);
  } catch (e) {
    toast(e.message.split('\n').slice(0, 3).join('\n'), 9000);
  }
  await reload().catch(() => {});
}

export async function checkout(name, label = name) {
  await run('/api/checkout', { ref: name }, `checking out ${label}…`);
  const { head } = state.repo;
  if (head && !state.entries.some(e => e.h === head) && !state.selected.includes('HEAD')) await setSelection([...state.selected, 'HEAD']);
  log.focus(head);
}

export async function deleteRemote(name) {
  const ok = await confirmAction(`Delete ${name} on the remote? Everyone who uses it loses the branch; BranchSight cannot undo it.`, 'Delete');
  if (ok) await run('/api/delete-remote', { ref: name }, `deleting ${name}…`);
}

export async function push(branch, upstream) {
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
  dialog.querySelector('[name="mode"][value="normal"]').checked = true;
  if ((await ask(dialog)) !== 'ok') return;
  const remote = select.value, mode = dialog.querySelector('[name="mode"]:checked').value;
  const lost = `Commits on ${remote}/${branch} that you do not have are lost for everyone who uses it.`;
  if (mode === 'force' && !(await confirmAction(`Force-push ${branch} to ${remote}? ${lost}`, 'Force push'))) return;
  await run('/api/push', { ref: branch, remote, mode }, `pushing ${branch} to ${remote}…`);
}

const currentBranch = () => state.repo.branch || 'HEAD';

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
