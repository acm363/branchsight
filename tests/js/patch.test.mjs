import assert from 'node:assert/strict';
import { test } from 'node:test';

import { patchHtml } from '../../src/web/changes/patch.js';

const patch = ['diff --git a/a.txt b/a.txt', '--- a/a.txt', '+++ b/a.txt', '@@ -1,2 +1,2 @@', ' same', '--- removed line', '+added', ''].join('\n');

test('lines are classed by where they sit, not only by their first character', () => {
  // When.
  const { html } = patchHtml(patch, 100);

  // Then.
  assert.match(html, /<span class="mt">--- a\/a\.txt<\/span>/);
  assert.match(html, /<span class="rm">--- removed line<\/span>/);
  assert.match(html, /<span class="ad">\+added<\/span>/);
  assert.match(html, /<span class="hk">@@/);
});

test('a second section of the same file starts over with its own headers', () => {
  // When.
  const { html } = patchHtml(`${patch}diff --git a/a.txt b/a.txt\n--- /dev/null\n+++ b/a.txt\n`, 100);

  // Then.
  assert.match(html, /<span class="mt">--- \/dev\/null<\/span>/);
});

test('lines past the budget are counted, not rendered', () => {
  // When.
  const { html, used, hidden } = patchHtml(patch, 3);

  // Then.
  assert.equal(used, 7);
  assert.equal(hidden, 4);
  assert.doesNotMatch(html, /@@/);
});

test('an empty patch renders nothing', () => {
  // Then.
  assert.deepEqual(patchHtml('', 10), { html: '', used: 0, hidden: 0 });
});
