import assert from 'node:assert/strict';
import { test } from 'node:test';

import { esc, fuzzy, highlight, samePath, shortPath } from '../../src/web/format.js';

test('a substring match ranks before a scattered one', () => {
  // When.
  const exact = fuzzy('main', 'feature/main-menu');
  const scattered = fuzzy('main', 'my-amazing-index');

  // Then.
  assert.ok(exact.score < scattered.score);
  assert.deepEqual(exact.marks, [8, 9, 10, 11]);
});

test('a query whose letters are not all present does not match', () => {
  // Then.
  assert.equal(fuzzy('xyz', 'main'), null);
});

test('highlighted text is escaped', () => {
  // When.
  const html = highlight('<a>', [1]);

  // Then.
  assert.equal(html, '&lt;<mark>a</mark>&gt;');
  assert.equal(esc('"&'), '&quot;&amp;');
});

test('paths compare the same with either separator and drive letter case', () => {
  // Then.
  assert.ok(samePath('C:/Users/me/code', 'c:\\Users\\me\\code\\'));
  assert.ok(samePath('/home/me/code', '/home/me/code/'));
  assert.ok(!samePath('/home/me/code', '/home/me/code2'));
  assert.ok(!samePath(null, '/home/me'));
});

test('a project path is shown relative to the root it was found under', () => {
  // Then.
  assert.equal(shortPath('/home/me/work/api', ['/home/me']), 'work/api');
  assert.equal(shortPath('C:/Users/me/work/api', ['C:\\Users\\me']), 'work/api');
  assert.equal(shortPath('/srv/api', ['/home/me']), '/srv/api');
});
