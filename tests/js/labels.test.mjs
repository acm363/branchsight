import assert from 'node:assert/strict';
import { test } from 'node:test';

import { indexRefs, refLabels, refName } from '../../src/web/labels.js';

const head = name => ({ kind: 'head', name, hash: 'a' });
const remote = (remoteName, branch) => ({ kind: 'remote', name: `${remoteName}/${branch}`, remote: remoteName, branch, hash: 'a' });
const tag = name => ({ kind: 'tag', name, hash: 'a' });

test('a remote branch with the same name joins its local branch label', () => {
  // When.
  const labels = refLabels([head('main'), remote('origin', 'main'), remote('team/origin', 'main')], { branch: 'main' });

  // Then.
  assert.equal(labels.length, 1);
  assert.equal(labels[0].current, true);
  assert.deepEqual(labels[0].remotes.map(r => r.remote), ['origin', 'team/origin']);
});

test('a remote branch whose name only ends like a local one keeps its own label', () => {
  // When.
  const labels = refLabels([head('main'), remote('origin', 'feature/main')], { branch: 'main' });

  // Then.
  assert.deepEqual(labels.map(l => [l.kind, l.name]), [['head', 'main'], ['remote', 'origin/feature/main']]);
});

test('a detached HEAD shows first, tags last', () => {
  // When.
  const labels = refLabels([tag('v1'), head('dev')], { branch: '', detached: true });

  // Then.
  assert.deepEqual(labels.map(l => l.name), ['HEAD', 'dev', 'v1']);
});

test('refs are grouped by commit, local branches before remotes and tags', () => {
  // When.
  const byHash = indexRefs([tag('v1'), remote('origin', 'x'), head('x'), { ...head('other'), hash: 'b' }]);

  // Then.
  assert.deepEqual(byHash.get('a').map(r => r.kind), ['head', 'remote', 'tag']);
  assert.equal(refName(byHash.get('a')), 'x');
  assert.equal(refName(undefined), undefined);
});
