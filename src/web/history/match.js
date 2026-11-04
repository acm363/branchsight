export function buildMatcher(query, { caseSensitive = false, regex = false } = {}) {
  if (!query) return null;
  if (regex) {
    let find, probe;
    try {
      find = new RegExp(query, caseSensitive ? 'g' : 'gi');
      probe = new RegExp(query, caseSensitive ? '' : 'i');
    } catch {
      return { invalid: true, test: () => false, ranges: () => [] };
    }
    return {
      test: s => probe.test(s),
      ranges: s => {
        const out = [];
        find.lastIndex = 0;
        for (let m; (m = find.exec(s)); ) {
          out.push([m.index, m.index + m[0].length]);
          if (m[0].length === 0) find.lastIndex++;
        }
        return out;
      },
    };
  }
  const needle = caseSensitive ? query : query.toLowerCase();
  const norm = s => (caseSensitive ? s : s.toLowerCase());
  return {
    test: s => norm(s).includes(needle),
    ranges: s => {
      const out = [], hay = norm(s);
      for (let at = hay.indexOf(needle); at >= 0; at = hay.indexOf(needle, at + needle.length)) out.push([at, at + needle.length]);
      return out;
    },
  };
}
