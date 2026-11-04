import { $, copy } from '../shared/dom.js';
import { SHORT_HASH, esc } from '../shared/format.js';
import { t } from '../shared/i18n.js';

export function refSuffix(source) {
  if (source.kind === 'commit') return ` @${source.h.slice(0, SHORT_HASH)}`;
  if (source.kind === 'compare') return ` @${source.to.slice(0, SHORT_HASH)}`;
  return '';
}

export function formatRef(path, a, b, source) {
  const at = p => (p.col ? `${p.line}:${p.col}` : `${p.line}`);
  if (b && b.col === 1 && b.line > a.line) b = { line: b.line - 1 };
  let loc;
  if (!b || (b.line === a.line && b.col === a.col)) loc = at(a);
  else if (b.line === a.line) loc = `${a.line}:${a.col}-${b.col}`;
  else loc = `${at(a)}-${at(b)}`;
  return `${path}:${loc}${refSuffix(source)}`;
}

export function showRefMenu(e, ref) {
  if (!ref) return;
  e.preventDefault();
  const menu = $('ctx');
  const items = [[t('copyref.ref'), () => copy(ref.text)]];
  if (ref.code) items.push([t('copyref.refCode'), () => copy(`${ref.text}\n${ref.code}`)]);
  menu.innerHTML = items.map(([label], i) => `<button data-i="${i}">${esc(label)}</button>`).join('');
  const onDown = ev => { if (!menu.contains(ev.target)) close(); };
  const onKey = ev => { if (ev.key === 'Escape') { ev.stopPropagation(); close(); } };
  function close() {
    menu.hidden = true;
    document.removeEventListener('mousedown', onDown, true);
    document.removeEventListener('keydown', onKey, true);
  }
  menu.onclick = ev => {
    const button = ev.target.closest('button[data-i]');
    if (!button) return;
    close();
    items[+button.dataset.i][1]();
  };
  menu.hidden = false;
  document.addEventListener('mousedown', onDown, true);
  document.addEventListener('keydown', onKey, true);
  const { width, height } = menu.getBoundingClientRect();
  menu.style.left = `${Math.min(e.clientX, window.innerWidth - width - 4)}px`;
  menu.style.top = `${Math.min(e.clientY, window.innerHeight - height - 4)}px`;
}
