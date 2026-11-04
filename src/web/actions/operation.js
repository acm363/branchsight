import { $, confirmAction } from '../shared/dom.js';
import { plural } from '../shared/format.js';
import { t } from '../shared/i18n.js';
import { onLog } from '../history/sync.js';
import { abortOperation, finishOperation } from './commands.js';

let current = null;

onLog(seen => {
  current = seen?.operation ?? null;
  $('operation').hidden = !current;
  if (!current) return;
  const conflicts = seen.status?.conflicts ?? 0;
  const next = conflicts
    ? t(conflicts > 1 ? 'op.conflictMany' : 'op.conflictOne', { files: plural(conflicts, 'file') })
    : t('op.continueToFinish');
  $('operation-text').textContent = t('op.inProgress', { what: t(`op.name.${current}`), next });
});

$('operation-continue').addEventListener('click', () => finishOperation(current));

$('operation-abort').addEventListener('click', async () => {
  if (await confirmAction(t('op.abortConfirm', { op: t(`op.the.${current}`) }), t('op.abort'))) abortOperation(current);
});
