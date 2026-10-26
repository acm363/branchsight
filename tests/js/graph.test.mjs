import assert from 'node:assert/strict';
import { test } from 'node:test';

import { DEFAULT_SELECTION, layout } from '../../src/web/graph.js';

const c = (h, ...p) => ({ h, p });

test('a linear history stays on one lane with one colour', () => {
  const { rows, lanes } = layout([c('c', 'b'), c('b', 'a'), c('a')]);
  assert.equal(lanes, 1);
  assert.deepEqual(rows.map(r => r.col), [0, 0, 0]);
  assert.equal(new Set(rows.map(r => r.color)).size, 1);
});

test('a merge opens a lane for its second parent and closes it where both meet', () => {
  const { rows, lanes } = layout([c('m', 'b', 't'), c('t', 'a'), c('b', 'a'), c('a')]);
  assert.equal(lanes, 2);
  assert.deepEqual(rows[0].out.map(([lane]) => lane), [0, 1]);
  assert.equal(rows[1].col, 1, 'topic commit sits on the second lane');
  assert.equal(rows[2].col, 0, 'first parent stays on the main lane');
  const intoRoot = rows[3].before.filter(h => h === 'a').length;
  assert.equal(intoRoot, 2, 'both lanes converge into the common ancestor');
  assert.equal(rows[3].col, 0);
});

test('the first parent keeps the lane colour, a branch gets its own', () => {
  const { rows } = layout([c('m', 'b', 't'), c('t', 'a'), c('b', 'a'), c('a')]);
  assert.equal(rows[2].color, rows[0].color);
  assert.notEqual(rows[1].color, rows[0].color);
});

test('a freed lane is reused instead of widening the graph', () => {
  const { lanes } = layout([
    c('m2', 'm1', 't2'), c('t2', 'm1'),
    c('m1', 'b', 't1'), c('t1', 'b'),
    c('b'),
  ]);
  assert.equal(lanes, 2);
});

test('parents outside the loaded window are flagged so the edge can be drawn as cut', () => {
  const { rows } = layout([c('b', 'a')]);
  assert.deepEqual(rows[0].out.map(([, , known]) => known), [false]);
});

test('several branch tips start on separate lanes', () => {
  const { rows, lanes } = layout([c('x', 'a'), c('y', 'a'), c('a')]);
  assert.equal(lanes, 2);
  assert.notEqual(rows[0].col, rows[1].col);
});

test('the default view selects every local and remote branch', () => {
  assert.deepEqual(DEFAULT_SELECTION, ['@local', '@remotes']);
});
