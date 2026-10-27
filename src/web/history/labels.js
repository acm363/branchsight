const KIND_ORDER = { head: 0, remote: 1, tag: 2 };

export function indexRefs(refs) {
  const byHash = new Map();
  [...refs].sort((a, b) => KIND_ORDER[a.kind] - KIND_ORDER[b.kind]).forEach(r => {
    if (!byHash.has(r.hash)) byHash.set(r.hash, []);
    byHash.get(r.hash).push(r);
  });
  return byHash;
}

export function refLabels(refs, { branch, detached }) {
  const remotes = refs.filter(r => r.kind === 'remote');
  const tracked = new Set();
  const local = refs.filter(r => r.kind === 'head').map(h => {
    const tracking = remotes.filter(r => r.branch === h.name);
    tracking.forEach(r => tracked.add(r));
    return { kind: 'head', name: h.name, current: h.name === branch, remotes: tracking, upstream: h.upstream };
  });
  const remoteOnly = remotes.filter(r => !tracked.has(r)).map(r => ({ kind: 'remote', name: r.name, remotes: [r] }));
  const tags = refs.filter(r => r.kind === 'tag').map(r => ({ kind: 'tag', name: r.name, remotes: [] }));
  const head = detached ? [{ kind: 'detached', name: 'HEAD', current: true, remotes: [] }] : [];
  return [...head, ...local, ...remoteOnly, ...tags];
}

export function refName(refs = []) {
  return (refs.find(r => r.kind === 'head') || refs.find(r => r.kind === 'remote') || refs.find(r => r.kind === 'tag'))?.name;
}

export const fileLabel = file => (file.from ? `${file.from} → ${file.path}` : file.path);
