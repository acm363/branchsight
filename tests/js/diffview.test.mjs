import assert from 'node:assert/strict';
import { test } from 'node:test';

import { buildDiff, folds } from '../../src/web/changes/diffview.js';

const diff = (...lines) => ['diff --git a/f b/f', 'index 1..2 100644', '--- a/f', '+++ b/f', ...lines].join('\n');

test('context lines sit on both sides with their own line numbers', () => {
  // Given.
  const text = diff('@@ -1,2 +1,2 @@', ' first', ' second');

  // When.
  const { split } = buildDiff(text);

  // Then.
  assert.deepEqual(split.map(r => [r.left.n, r.right.n, r.left.text]), [[1, 1, 'first'], [2, 2, 'second']]);
});

test('a replaced line pairs the old version on the left with the new one on the right', () => {
  // Given.
  const text = diff('@@ -1,3 +1,3 @@', ' keep', '-return input.trim();', '+return input.trim().toLowerCase();', ' end');

  // When.
  const { split, changes } = buildDiff(text);

  // Then.
  const changed = split[1];
  assert.equal(changed.left.text, 'return input.trim();');
  assert.equal(changed.right.text, 'return input.trim().toLowerCase();');
  assert.deepEqual([changed.left.n, changed.right.n], [2, 2]);
  assert.equal(changes.length, 1);
  assert.equal(changes[0].split, 1);
});

test('the changed part inside a modified line is isolated for highlighting', () => {
  // Given.
  const text = diff('@@ -1 +1 @@', '-const limit = 5000;', '+const limit = 20000;');

  // When.
  const { split } = buildDiff(text);

  // Then.
  const { left, right } = split[0];
  assert.equal(left.text.slice(...left.range), '5');
  assert.equal(right.text.slice(...right.range), '20');
});

test('more additions than deletions leave blank cells on the left', () => {
  // Given.
  const text = diff('@@ -1,1 +1,3 @@', '-a', '+b', '+c', '+d');

  // When.
  const { split } = buildDiff(text);

  // Then.
  assert.deepEqual(split.map(r => [r.left?.text ?? null, r.right?.text ?? null]), [['a', 'b'], [null, 'c'], [null, 'd']]);
});

test('separate blocks of changes are each a stop for next and previous', () => {
  // Given.
  const text = diff('@@ -1,5 +1,5 @@', '-one', '+ONE', ' two', ' three', '-four', '+FOUR', ' five');

  // When.
  const { changes, unified } = buildDiff(text);

  // Then.
  assert.equal(changes.length, 2);
  assert.deepEqual(changes.map(c => unified[c.unified].text), ['one', 'four']);
});

test('the unified view lists deletions before additions, numbered on their own side', () => {
  // Given.
  const text = diff('@@ -3,2 +3,2 @@', '-old', '+new', ' same');

  // When.
  const { unified } = buildDiff(text);

  // Then.
  assert.deepEqual(unified.map(r => [r.kind, r.old, r.new]), [['del', 3, null], ['add', null, 3], ['ctx', 4, 4]]);
});

test('a new file has nothing on the left and a binary file is flagged', () => {
  // Given.
  const added = ['diff --git a/n b/n', 'new file mode 100644', '--- /dev/null', '+++ b/n', '@@ -0,0 +1,2 @@', '+x', '+y'].join('\n');
  const binary = ['diff --git a/i.png b/i.png', 'Binary files a/i.png and b/i.png differ'].join('\n');

  // When.
  const created = buildDiff(added), image = buildDiff(binary);

  // Then.
  assert.ok(created.split.every(r => r.left === null && r.right.kind === 'add'));
  assert.equal(image.binary, true);
  assert.equal(image.split.length, 0);
});

const rows = pattern => [...pattern].map(c => c === '.');

test('unchanged lines away from a change fold, keeping three lines around it', () => {
  // Given.
  const plain = rows('..........x..........');

  // When.
  const runs = folds(plain);

  // Then.
  assert.deepEqual(runs, [{ from: 0, to: 7 }, { from: 14, to: 21 }]);
});

test('a run between two changes keeps three lines on each side', () => {
  // Given.
  const plain = rows('x..........x');

  // When.
  const runs = folds(plain);

  // Then.
  assert.deepEqual(runs, [{ from: 4, to: 8 }]);
});

test('a run too short to save space is left open', () => {
  // Given.
  const plain = rows('x.........x.......');

  // When.
  const runs = folds(plain);

  // Then.
  assert.deepEqual(runs, [{ from: 14, to: 18 }]);
});
