import assert from 'node:assert/strict';
import { test } from 'node:test';

import { buildMatcher } from '../../src/web/history/match.js';

test('an empty query builds no matcher', () => {
  assert.equal(buildMatcher(''), null);
});

test('a substring match ignores case by default', () => {
  // Given.
  const m = buildMatcher('Stag');

  // Then.
  assert.ok(m.test('chore(lock): upgrade on gcp-staging'));
  assert.deepEqual(m.ranges('gcp-staging'), [[4, 8]]);
});

test('every occurrence is reported for highlighting', () => {
  // When.
  const ranges = buildMatcher('a').ranges('banana');

  // Then.
  assert.deepEqual(ranges, [[1, 2], [3, 4], [5, 6]]);
});

test('case-sensitive search distinguishes case', () => {
  // Given.
  const m = buildMatcher('Fix', { caseSensitive: true });

  // Then.
  assert.ok(m.test('Fix: keycloak'));
  assert.ok(!m.test('fix: keycloak'));
});

test('a regex matches and reports its ranges', () => {
  // Given.
  const m = buildMatcher('SCRUM-\\d+', { regex: true });

  // Then.
  assert.ok(m.test('conf(invoiceRequest): SCRUM-4564'));
  assert.deepEqual(m.ranges('SCRUM-4564 and SCRUM-91'), [[0, 10], [15, 23]]);
});

test('a zero-width regex does not loop forever', () => {
  // When.
  const ranges = buildMatcher('x*', { regex: true }).ranges('ab');

  // Then.
  assert.equal(ranges.length, 3);
});

test('an invalid regex matches nothing and is flagged', () => {
  // Given.
  const m = buildMatcher('(', { regex: true });

  // Then.
  assert.ok(m.invalid);
  assert.ok(!m.test('anything'));
  assert.deepEqual(m.ranges('anything'), []);
});
