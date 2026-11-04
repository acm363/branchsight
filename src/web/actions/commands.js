import { post } from '../shared/api.js';
import { setSelection } from '../header/branches.js';
import { $, ask, confirmAction, toast } from '../shared/dom.js';
import { SHORT_HASH, esc, plural } from '../shared/format.js';
import { t } from '../shared/i18n.js';
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
  await run('/api/checkout', { ref: name }, t('busy.checkout', { name: label }));
}

export async function deleteRemote(name) {
  if (await confirmAction(t('cmd.deleteRemoteConfirm', { name }), t('btn.delete'))) await run('/api/delete-remote', { ref: name }, t('busy.deleting', { name }), null);
}

export async function deleteLocal(name) {
  if (!(await confirmAction(t('cmd.deleteLocalConfirm', { name }), t('btn.delete')))) return;
  try {
    const { out } = await post('/api/delete-local', { ref: name });
    toast(out, 3000);
    await reload().catch(() => {});
  } catch (e) {
    if (!/not fully merged/i.test(e.message)) {
      toast(e.message.split('\n').slice(0, 3).join('\n'), 9000);
      return;
    }
    if (await confirmAction(t('cmd.deleteLocalUnmerged', { name }), t('btn.deleteAnyway'))) await run('/api/delete-local', { ref: name, force: '1' }, t('busy.deleting', { name }), null);
  }
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
    $('push-note').textContent = t('cmd.pushNote', { branch, current: upstream?.name, target });
  };
  note();
  select.onchange = note;
  dialog.onkeydown = enterOnlyOnButtons;
  dialog.querySelector(`[name="mode"][value="${initialMode}"]`).checked = true;
  if ((await ask(dialog)) !== 'ok') return;
  const remote = select.value, mode = dialog.querySelector('[name="mode"]:checked').value;
  const lost = t('cmd.pushForceLost', { remote, branch });
  if (mode === 'force' && !(await confirmAction(t('cmd.forcePushConfirm', { branch, remote, lost }), t('btn.forcePush')))) return;
  await run('/api/push', { ref: branch, remote, mode }, t('busy.pushing', { branch, remote }), null);
}

const currentBranch = () => state.repo.branch || 'HEAD';

export async function resetToRemote(branch, remote, own) {
  if (await confirmAction(t('cmd.resetToRemoteConfirm', { branch, remote, commits: plural(own, 'commit') }), t('btn.reset'))) await run('/api/reset-to-remote', { branch, from: remote }, t('busy.resettingToRemote', { branch, remote }), tipOf(branch));
}

export async function pull(branch, from) {
  const dialog = $('pull'), elsewhere = branch !== state.repo.branch;
  $('pull-from').textContent = from;
  $('pull-branch').textContent = branch;
  $('pull-note').hidden = !elsewhere;
  $('pull-note').textContent = t('cmd.pullNote', { branch });
  dialog.querySelector('[name="mode"][value="merge"]').checked = true;
  dialog.onkeydown = enterOnlyOnButtons;
  if ((await ask(dialog)) !== 'ok') return;
  const mode = dialog.querySelector('[name="mode"]:checked').value;
  await run('/api/pull', { branch, from, mode }, t('busy.syncing', { branch, from }), tipOf(branch));
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

async function apply(endpoint, verbKey, commit) {
  const dialog = $('apply'), noCommit = $('apply-no-commit'), short = commit.h.slice(0, SHORT_HASH), verb = t(`verb.${verbKey}`);
  $('apply-text').textContent = t('cmd.applyText', { verb, short, subject: commit.s, branch: currentBranch() });
  $('apply-ok').textContent = verb;
  noCommit.checked = false;
  dialog.onkeydown = enterOnlyOnButtons;
  if ((await ask(dialog)) !== 'ok') return;
  await run(endpoint, { h: commit.h, commit: noCommit.checked ? '0' : '1' }, t('busy.applying', { verb, short }));
}

export const cherryPick = commit => apply('/api/cherry-pick', 'cherryPick', commit);

export const revert = commit => apply('/api/revert', 'revert', commit);

export async function drop(commit) {
  const short = commit.h.slice(0, SHORT_HASH), into = currentBranch();
  if (await confirmAction(t('cmd.dropConfirm', { short, subject: commit.s, branch: into }), t('btn.drop'))) await run('/api/drop', { h: commit.h }, t('busy.dropping', { short }));
}

export async function rebase(name, shown = name) {
  const into = currentBranch();
  if (await confirmAction(t('cmd.rebaseConfirm', { into, shown }), t('btn.rebase'))) await run('/api/rebase', { ref: name }, t('busy.rebasing', { into, shown }));
}

export async function reset(commit) {
  const dialog = $('reset'), short = commit.h.slice(0, SHORT_HASH), into = currentBranch();
  $('reset-branch').textContent = into;
  $('reset-commit').textContent = `${short} "${commit.s}"`;
  dialog.querySelector('[name="mode"][value="mixed"]').checked = true;
  dialog.onkeydown = enterOnlyOnButtons;
  if ((await ask(dialog)) !== 'ok') return;
  const mode = dialog.querySelector('[name="mode"]:checked').value;
  const lost = t('cmd.resetHardLost', { short });
  if (mode === 'hard' && !(await confirmAction(t('cmd.resetHardConfirm', { into, short, lost }), t('btn.discardReset')))) return;
  await run('/api/reset', { h: commit.h, mode }, t('busy.resetting', { into, short }));
}

export async function fetchRemotes({ unattended = false } = {}) {
  if (fetching || !state.repo.path) return;
  fetching = true;
  const button = $('fetch');
  button.disabled = true;
  button.textContent = t('cmd.fetching');
  try {
    await post('/api/fetch', unattended ? { auto: '1' } : {});
    button.title = t('cmd.fetchLast', { time: new Date().toLocaleTimeString() });
    button.classList.remove('err');
  } catch (e) {
    button.title = t('cmd.fetchFailed', { msg: e.message });
    button.classList.add('err');
    toast(t('cmd.fetchFailedToast', { msg: e.message.split('\n')[0] }), 5000);
  } finally {
    lastFetch = Date.now();
    fetching = false;
    button.disabled = false;
    button.textContent = t('btn.fetch');
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
