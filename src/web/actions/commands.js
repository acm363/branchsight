import { post } from '../shared/api.js';
import { setSelection } from '../header/branches.js';
import { $, ask, confirmAction, toast } from '../shared/dom.js';
import { esc } from '../shared/format.js';
import * as log from '../history/list.js';
import { state } from '../shared/state.js';
import { poll, reload } from '../history/sync.js';

const AUTO_FETCH_MS = 120000, AUTO_FETCH_CHECK_MS = 10000;

let fetching = false, lastFetch = 0;

async function run(endpoint, params, busy) {
  toast(busy, 60000);
  try {
    const { out } = await post(endpoint, params);
    toast(out, 3000);
  } catch (e) {
    toast(e.message.split('\n')[0], 7000);
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
  dialog.querySelector('[name="mode"][value="normal"]').checked = true;
  if ((await ask(dialog)) !== 'ok') return;
  const remote = $('push-remote').value, mode = dialog.querySelector('[name="mode"]:checked').value;
  const lost = `Commits on ${remote}/${branch} that you do not have are lost for everyone who uses it.`;
  if (mode === 'force' && !(await confirmAction(`Force-push ${branch} to ${remote}? ${lost}`, 'Force push'))) return;
  await run('/api/push', { ref: branch, remote, mode }, `pushing ${branch} to ${remote}…`);
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

$('fetch').addEventListener('click', fetchRemotes);
$('auto-fetch').addEventListener('change', e => {
  state.store.set('autoFetch', e.target.checked);
  autoFetch();
});
