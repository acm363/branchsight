import { esc } from './format.js';

export function patchSections(patch, budget) {
  const sections = [];
  let inHunk = false, hidden = 0;
  for (const line of patch.replace(/\n$/, '').split('\n')) {
    if (line.startsWith('diff --git ')) {
      sections.push('');
      inHunk = false;
    }
    if (!sections.length) continue;
    if (budget <= 0) {
      hidden++;
      continue;
    }
    budget--;
    inHunk ||= line.startsWith('@@');
    const cls = lineClass(line, inHunk);
    sections[sections.length - 1] += `<span${cls ? ` class="${cls}"` : ''}>${esc(line) || ' '}</span>`;
  }
  return { sections, hidden };
}

function lineClass(line, inHunk) {
  if (!inHunk || line[0] === '\\') return 'mt';
  return { '@': 'hk', '+': 'ad', '-': 'rm' }[line[0]] || '';
}
