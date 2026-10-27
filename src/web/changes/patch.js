import { esc } from '../shared/format.js';

export const lineHtml = (text, cls = '') => `<span${cls ? ` class="${cls}"` : ''}>${esc(text) || ' '}</span>`;

export function patchHtml(patch, budget) {
  const lines = patch ? patch.replace(/\n$/, '').split('\n') : [];
  let html = '', inHunk = false;
  for (const line of lines.slice(0, Math.max(0, budget))) {
    if (line.startsWith('diff ')) inHunk = false;
    inHunk ||= line.startsWith('@@');
    html += lineHtml(line, lineClass(line, inHunk));
  }
  return { html, used: lines.length, hidden: Math.max(0, lines.length - budget) };
}

function lineClass(line, inHunk) {
  if (!inHunk || line[0] === '\\') return 'mt';
  return { '@': 'hk', '+': 'ad', '-': 'rm' }[line[0]] || '';
}
