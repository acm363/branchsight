import { getJson, post } from '../shared/api.js';
import { $, ask, confirmAction, toast } from '../shared/dom.js';
import { SHORT_HASH, esc, fileLabel, plural, statusClass } from '../shared/format.js';
import { state } from '../shared/state.js';
import { reload } from '../history/sync.js';

const ACTION_MS = 15 * 60 * 1000;
const pathParams = paths => paths.map(p => ['path', p]);
const fileWord = paths => (paths.length === 1 ? paths[0] : plural(paths.length, 'file'));
const remotes = () => state.repo.remotes ?? [];
const submitOnEnter = dialog => e => {
  if (e.key !== 'Enter' || e.target.tagName === 'TEXTAREA') return;
  e.preventDefault();
  dialog.querySelector('[value="ok"]').click();
};

async function act(endpoint, params, busy) {
  toast(busy, ACTION_MS);
  let ok = false;
  try {
    const { out } = await post(endpoint, params);
    toast(out, 3000);
    ok = true;
  } catch (e) {
    toast(e.message.split('\n').slice(0, 3).join('\n'), 9000);
  }
  await reload().catch(() => {});
  return ok;
}

export const stage = paths => act('/api/stage', pathParams(paths), `staging ${fileWord(paths)}…`);

export const unstage = paths => act('/api/unstage', pathParams(paths), `unstaging ${fileWord(paths)}…`);

export async function discard(paths, { all = false } = {}) {
  const what = all ? 'every uncommitted change' : fileWord(paths);
  const text = `Discard ${what}? Tracked files return to the last commit and new files are deleted; BranchSight cannot undo it.`;
  if (await confirmAction(text, 'Discard')) await act('/api/discard', all ? { all: '1' } : pathParams(paths), `discarding ${what}…`);
}

export async function commit() {
  const worktree = await getJson('/api/worktree').catch(() => null);
  const staged = worktree?.staged?.files ?? [];
  const dialog = $('commit'), message = $('commit-message'), amend = $('commit-amend');
  const head = state.repo.branch && state.refs.find(r => r.kind === 'head' && r.name === state.repo.branch);
  const pushed = Boolean(head?.upstream) && (head.ahead ?? 0) === 0;
  const headSubject = state.entries.find(e => e.h === state.repo.head)?.s ?? '';
  const renderFiles = () => {
    $('commit-files').innerHTML = staged.length
      ? staged.map(f => `<div class="cfile"><span class="st ${statusClass(f)}">${f.status}</span><span class="p">${esc(fileLabel(f))}</span></div>`).join('')
      : '<div class="cfile none">nothing staged</div>';
    $('commit-count').textContent = plural(staged.length, 'staged file');
  };
  const warn = () => {
    $('commit-amend-note').hidden = !(amend.checked && pushed);
  };
  message.value = '';
  amend.checked = false;
  amend.disabled = !state.repo.head;
  renderFiles();
  warn();
  amend.onchange = () => {
    if (amend.checked && !message.value.trim()) message.value = headSubject;
    warn();
  };
  message.onkeydown = e => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      dialog.querySelector('[value="ok"]').click();
    }
  };
  dialog.onkeydown = e => {
    if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey && e.target.tagName !== 'TEXTAREA') e.preventDefault();
  };
  if ((await ask(dialog)) !== 'ok') return;
  const text = message.value.trim();
  if (!text) return void toast('a commit message is required', 3000);
  const amending = amend.checked;
  await act('/api/commit-staged', amending ? { message: text, amend: '1' } : { message: text }, amending ? 'amending the last commit…' : 'committing…');
}

export async function createTag(commit) {
  const dialog = $('tag'), name = $('tag-name'), message = $('tag-message'), push = $('tag-push'), select = $('tag-remote');
  $('tag-commit').textContent = `${commit.h.slice(0, SHORT_HASH)} "${commit.s}"`;
  name.value = message.value = '';
  push.checked = false;
  $('tag-push-row').hidden = !remotes().length;
  select.innerHTML = remotes().map(r => `<option>${esc(r)}</option>`).join('');
  select.value = remotes().includes('origin') ? 'origin' : remotes()[0];
  dialog.onkeydown = submitOnEnter(dialog);
  if ((await ask(dialog)) !== 'ok') return;
  const tag = name.value.trim();
  if (!tag) return;
  const made = await act('/api/tag', { name: tag, h: commit.h, message: message.value.trim() }, `creating tag ${tag}…`);
  if (made && push.checked && remotes().length) await act('/api/push-tag', { name: tag, remote: select.value }, `pushing tag ${tag} to ${select.value}…`);
}

export async function pushTag(name) {
  if (!remotes().length) return void toast('no remote to push to', 3000);
  const dialog = $('pushtag'), select = $('pushtag-remote');
  $('pushtag-name').textContent = name;
  select.innerHTML = remotes().map(r => `<option>${esc(r)}</option>`).join('');
  select.value = remotes().includes('origin') ? 'origin' : remotes()[0];
  dialog.onkeydown = submitOnEnter(dialog);
  if ((await ask(dialog)) !== 'ok') return;
  await act('/api/push-tag', { name, remote: select.value }, `pushing tag ${name} to ${select.value}…`);
}

export async function deleteTag(name) {
  if (await confirmAction(`Delete tag ${name}? It is removed locally; a copy already pushed to a remote stays there.`, 'Delete')) {
    await act('/api/delete-tag', { name }, `deleting tag ${name}…`);
  }
}

export async function createBranch(commit) {
  const dialog = $('branch'), name = $('branch-name');
  $('branch-commit').textContent = `${commit.h.slice(0, SHORT_HASH)} "${commit.s}"`;
  name.value = '';
  dialog.onkeydown = submitOnEnter(dialog);
  if ((await ask(dialog)) !== 'ok') return;
  const branch = name.value.trim();
  if (branch) await act('/api/branch', { name: branch, h: commit.h }, `creating branch ${branch}…`);
}
