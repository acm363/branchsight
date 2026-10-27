const COLORS = ['#3b82f6', '#e5484d', '#30a46c', '#f5a524', '#8e4ec6', '#12a594', '#d6409f', '#0090ff', '#ad7f58', '#7c66dc', '#e16f24', '#5bb98b'];

export const SHORT_HASH = 7;

export const color = n => COLORS[n % COLORS.length];

export const clamp = (value, min, max) => Math.max(min, Math.min(max, value));

export const esc = s => s.replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

export const fileLabel = file => (file.from ? `${file.from} → ${file.path}` : file.path);

export const statusClass = file => `st-${file.status === '?' ? 'new' : file.status}`;

export const plural = (n, word) => `${n.toLocaleString()} ${word}${n === 1 ? '' : 's'}`;

export function fmtDate(t) {
  const d = new Date(t * 1000), p = n => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

export function fuzzy(query, text) {
  if (!query) return { score: 0, marks: [] };
  const hay = text.toLowerCase(), needle = query.toLowerCase();
  const at = hay.indexOf(needle);
  if (at >= 0) return { score: at - 1000 + text.length / 100, marks: [...needle].map((_, k) => at + k) };
  let score = 0, last = -1;
  const marks = [];
  for (const ch of needle) {
    const i = hay.indexOf(ch, last + 1);
    if (i < 0) return null;
    score += i - last - 1;
    marks.push(i);
    last = i;
  }
  return { score: score + text.length / 100, marks };
}

export function highlight(text, marks) {
  const marked = new Set(marks);
  return [...text].map((ch, i) => marked.has(i) ? `<mark>${esc(ch)}</mark>` : esc(ch)).join('');
}

export const pathKey = path => path.replace(/\\/g, '/').replace(/^[a-z]:/, d => d.toUpperCase()).replace(/\/+$/, '');

export const samePath = (a, b) => a != null && b != null && pathKey(a) === pathKey(b);

export function shortPath(path, roots) {
  const key = pathKey(path);
  const root = roots.map(pathKey).find(r => key.startsWith(`${r}/`));
  return root === undefined ? path : key.slice(root.length + 1);
}
