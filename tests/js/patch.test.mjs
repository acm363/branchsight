import assert from 'node:assert/strict';
import { test } from 'node:test';

import { patchSections } from '../../src/web/patch.js';

const patch = [
  'diff --git a/a.txt b/a.txt', '--- a/a.txt', '+++ b/a.txt', '@@ -1,2 +1,2 @@', ' same', '--- removed line', '+added',
  'diff --git a/logo.png b/logo.png', 'Binary files a/logo.png and b/logo.png differ', '',
].join('\n');

test('each file of the patch gets its own section, in order', () => {
  // When.
  const { sections, hidden } = patchSections(patch, 100);

  // Then.
  assert.equal(sections.length, 2);
  assert.match(sections[0], /a\.txt/);
  assert.match(sections[1], /Binary files/);
  assert.equal(hidden, 0);
});

test('lines are classed by where they sit, not only by their first character', () => {
  // When.
  const [text] = patchSections(patch, 100).sections;

  // Then.
  assert.match(text, /<span class="mt">--- a\/a\.txt<\/span>/);
  assert.match(text, /<span class="rm">--- removed line<\/span>/);
  assert.match(text, /<span class="ad">\+added<\/span>/);
  assert.match(text, /<span class="hk">@@/);
});

test('lines past the budget are counted, and later files still get a section', () => {
  // When.
  const { sections, hidden } = patchSections(patch, 3);

  // Then.
  assert.equal(sections.length, 2);
  assert.equal(sections[1], '');
  assert.equal(hidden, 6);
});
