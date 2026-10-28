import { $, confirmAction } from '../shared/dom.js';
import { plural } from '../shared/format.js';
import { onLog } from '../history/sync.js';
import { abortOperation, finishOperation } from './commands.js';

const NAMES = { merge: 'A merge', rebase: 'A rebase', 'cherry-pick': 'A cherry-pick', revert: 'A revert' };

let current = null;

onLog(seen => {
  current = seen?.operation ?? null;
  $('operation').hidden = !current;
  if (!current) return;
  const conflicts = seen.status?.conflicts ?? 0;
  const next = conflicts
    ? `${plural(conflicts, 'file')} still in conflict: resolve ${conflicts > 1 ? 'them' : 'it'} in your editor, then continue`
    : 'continue to finish it';
  $('operation-text').textContent = `${NAMES[current]} is in progress; ${next}.`;
});

$('operation-continue').addEventListener('click', () => finishOperation(current));

$('operation-abort').addEventListener('click', async () => {
  const text = `Abort the ${current}? The branch goes back to where it was before it started, and conflicts you resolved are lost.`;
  if (await confirmAction(text, 'Abort')) abortOperation(current);
});
