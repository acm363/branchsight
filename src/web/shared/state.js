import { repoPrefs } from './prefs.js';

export const state = {
  roots: [],
  repo: {},
  store: repoPrefs(''),
  refs: [],
  refsByHash: new Map(),
  selected: [],
  entries: [],
  rows: [],
  lanes: 1,
  cursor: -1,
  compareHash: null,
};

export const isWorktree = entry => entry?.kind === 'worktree';

export const commitAt = i => isWorktree(state.entries[i]) ? null : state.entries[i] ?? null;
