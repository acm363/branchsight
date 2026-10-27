export const GROUPS = [
  { name: '@local', label: 'all local branches', kind: 'head' },
  { name: '@remotes', label: 'all remote branches', kind: 'remote' },
  { name: 'HEAD', label: 'HEAD, the checked-out commit', kind: 'head' },
];

export const DEFAULT_SELECTION = ['@local', '@remotes'];

export function layout(commits) {
  const known = new Set(commits.map(c => c.h));
  const open = [], colors = [];
  let next = 0;
  let lanes = 1;
  const rows = commits.map(c => {
    const before = open.slice(), bc = colors.slice();
    let col = open.indexOf(c.h);
    if (col < 0) {
      col = open.indexOf(null);
      if (col < 0) col = open.length;
      colors[col] = next++;
    }
    const nodeColor = colors[col];
    for (let j = 0; j < open.length; j++) if (open[j] === c.h) open[j] = null;
    const out = [];
    c.p.forEach((p, k) => {
      let t = k === 0 ? col : open.indexOf(p);
      if (k === 0) {
        colors[t] = nodeColor;
        open[t] = p;
      } else if (t < 0) {
        t = open.indexOf(null);
        if (t < 0) t = open.length;
        colors[t] = next++;
        open[t] = p;
      }
      out.push([t, colors[t], known.has(p)]);
    });
    while (open.length && open[open.length - 1] == null) { open.pop(); colors.pop(); }
    lanes = Math.max(lanes, before.length, open.length, col + 1);
    return { col, color: nodeColor, before, bc, out };
  });
  return { rows, lanes };
}

