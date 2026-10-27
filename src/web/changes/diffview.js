const HUNK = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;
const COMBINED_HUNK = /^@@@ -(\d+)(?:,\d+)? -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@@/;

function againstFirstParent(lines) {
  let combined = false;
  return lines.flatMap(line => {
    const hunk = line.match(COMBINED_HUNK);
    if (hunk) {
      combined = true;
      return [`@@ -${hunk[1]} +${hunk[2]} @@`];
    }
    if (line.startsWith('diff ') || line.startsWith('@@ ')) combined = false;
    if (!combined || line.startsWith('\\')) return [line];
    const [ours, theirs, rest] = [line[0], line[1], line.slice(2)];
    if (ours === '-' || ours === '+') return [ours + rest];
    return theirs === '-' ? [] : [` ${rest}`];
  });
}

function inlineRanges(a, b) {
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let end = 0;
  while (end < a.length - start && end < b.length - start && a[a.length - 1 - end] === b[b.length - 1 - end]) end++;
  return [[start, a.length - end], [start, b.length - end]];
}

export function buildDiff(text) {
  const split = [], unified = [], changes = [];
  let oldNo = 0, newNo = 0, inHunk = false, dels = [], adds = [];
  const binary = /^Binary files .* differ$/m.test(text);

  const flush = () => {
    if (!dels.length && !adds.length) return;
    changes.push({ split: split.length, unified: unified.length });
    for (let i = 0; i < Math.max(dels.length, adds.length); i++) {
      const left = dels[i] || null, right = adds[i] || null;
      if (left && right) [left.range, right.range] = inlineRanges(left.text, right.text);
      split.push({ left, right, change: true });
    }
    dels.forEach(d => unified.push({ old: d.n, new: null, text: d.text, kind: 'del', range: d.range }));
    adds.forEach(a => unified.push({ old: null, new: a.n, text: a.text, kind: 'add', range: a.range }));
    dels = [];
    adds = [];
  };

  for (const line of againstFirstParent(text.split('\n'))) {
    const hunk = line.match(HUNK);
    if (hunk) {
      flush();
      if (inHunk) {
        split.push({ gap: true });
        unified.push({ gap: true });
      }
      [oldNo, newNo] = [+hunk[1], +hunk[2]];
      inHunk = true;
      continue;
    }
    if (!inHunk || line.startsWith('\\')) continue;
    const body = line.slice(1);
    if (line[0] === '-') dels.push({ n: oldNo++, text: body, kind: 'del' });
    else if (line[0] === '+') adds.push({ n: newNo++, text: body, kind: 'add' });
    else if (line[0] === ' ') {
      flush();
      const n = { old: oldNo++, new: newNo++ };
      split.push({ left: { n: n.old, text: body, kind: 'ctx' }, right: { n: n.new, text: body, kind: 'ctx' }, change: false });
      unified.push({ old: n.old, new: n.new, text: body, kind: 'ctx' });
    }
  }
  flush();
  return { split, unified, changes, binary };
}
