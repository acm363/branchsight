import { $, drag, toast } from '../shared/dom.js';
import { prefs } from '../shared/prefs.js';
import { state } from '../shared/state.js';
import { termShells, termOpen, termWrite, termResize, termClose, newChannel } from '../shared/api.js';

const DOCKS = ['bottom', 'right', 'left'];
const terms = new Map();
let nextId = 1, activeId = null, visible = false, loading, shellsFilled, observer;

const dock = () => (DOCKS.includes(prefs.get('termDock')) ? prefs.get('termDock') : 'bottom');
const cssVar = name => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

const theme = () => ({
  background: cssVar('--bg2'),
  foreground: cssVar('--fg'),
  cursor: cssVar('--fg'),
  selectionBackground: cssVar('--sel'),
});

function loadXterm() {
  if (loading) return loading;
  loading = new Promise((resolve, reject) => {
    const link = Object.assign(document.createElement('link'), { rel: 'stylesheet', href: 'vendor/xterm.css' });
    document.head.append(link);
    const load = (src, next) => {
      const script = Object.assign(document.createElement('script'), { src, onload: next, onerror: reject });
      document.head.append(script);
    };
    load('vendor/xterm.js', () => load('vendor/addon-fit.js', resolve));
  });
  return loading;
}

async function fillShells() {
  if (shellsFilled) return;
  const select = $('term-shell');
  const list = await termShells();
  const chosen = prefs.get('termShell');
  select.replaceChildren(...list.map(shell => {
    const option = Object.assign(document.createElement('option'), { value: shell.id, textContent: shell.label });
    option.selected = shell.id === chosen;
    return option;
  }));
  select.hidden = list.length < 2;
  shellsFilled = true;
}

function renderTabs() {
  $('term-tabs').replaceChildren(...[...terms.values()].map(t => {
    const tab = document.createElement('button');
    tab.className = 'term-tab' + (t.id === activeId ? ' on' : '');
    tab.dataset.id = t.id;
    tab.append(
      Object.assign(document.createElement('span'), { className: 'term-tab-name', textContent: t.label }),
      Object.assign(document.createElement('span'), { className: 'term-tab-x', textContent: '✕' }),
    );
    return tab;
  }));
}

function fitActive() {
  const t = terms.get(activeId);
  if (!visible || !t) return;
  t.fit.fit();
  termResize(t.id, t.term.cols, t.term.rows);
}

function activate(id) {
  activeId = id;
  terms.forEach(t => { t.el.hidden = t.id !== id; });
  renderTabs();
  const t = terms.get(id);
  if (t) {
    fitActive();
    t.term.focus();
  }
}

async function createTerm() {
  await loadXterm();
  await fillShells();
  const id = nextId++;
  const el = Object.assign(document.createElement('div'), { className: 'term-view' });
  $('term-body').append(el);
  const term = new window.Terminal({
    fontFamily: cssVar('--mono') || 'monospace',
    fontSize: parseInt(cssVar('--fs'), 10) || 13,
    cursorBlink: true,
    theme: theme(),
  });
  const fit = new window.FitAddon.FitAddon();
  term.loadAddon(fit);
  term.open(el);
  fit.fit();
  term.onData(data => termWrite(id, data));
  const channel = newChannel();
  channel.onmessage = chunk => term.write(Uint8Array.from(atob(chunk), c => c.charCodeAt(0)));
  const label = `${$('term-shell').selectedOptions[0]?.textContent || 'shell'} ${id}`;
  try {
    await termOpen(id, prefs.get('termShell') || undefined, term.cols || 80, term.rows || 24, state.repo.path || undefined, channel);
  } catch (e) {
    term.dispose();
    el.remove();
    toast(String(e?.message ?? e));
    return;
  }
  terms.set(id, { id, term, fit, el, label });
  activate(id);
}

async function closeTerm(id) {
  const t = terms.get(id);
  if (!t) return;
  try {
    await termClose(id);
  } catch {}
  t.term.dispose();
  t.el.remove();
  terms.delete(id);
  if (terms.size === 0) {
    nextId = 1;
    activeId = null;
    hideTerminal();
  } else if (activeId === id) activate([...terms.keys()].at(-1));
  else renderTabs();
}

async function showPanel(create) {
  $('term').hidden = $('term-split').hidden = false;
  $('term-btn').classList.add('on');
  visible = true;
  applyTermDock();
  try {
    await loadXterm();
  } catch {
    hideTerminal();
    toast('failed to load the terminal');
    return;
  }
  if (!observer) {
    observer = new ResizeObserver(() => fitActive());
    observer.observe($('term-body'));
  }
  if (create && terms.size === 0) await createTerm();
  else {
    fitActive();
    terms.get(activeId)?.term.focus();
  }
}

export function hideTerminal() {
  visible = false;
  $('term').hidden = $('term-split').hidden = true;
  $('term-btn').classList.remove('on');
}

export function toggleTerminal() {
  return visible ? hideTerminal() : showPanel(true);
}

export async function newTerminal() {
  if (!visible) await showPanel(false);
  await createTerm();
}

export function applyTermDock() {
  const main = $('main');
  DOCKS.forEach(d => main.classList.toggle(`term-${d}`, d === dock()));
  const node = $('term');
  if (dock() === 'bottom') {
    node.style.width = '';
    const h = state.store.get('termHeight');
    node.style.height = h ? `${h}px` : '';
  } else {
    node.style.height = '';
    const w = state.store.get('termWidth');
    node.style.width = w ? `${w}px` : '';
  }
  fitActive();
}

$('term-split').addEventListener('mousedown', e => drag(e, ev => {
  const node = $('term'), main = $('main');
  if (dock() === 'bottom') {
    node.style.height = `${Math.max(120, Math.min(window.innerHeight - ev.clientY, main.clientHeight - 80))}px`;
  } else {
    const box = main.getBoundingClientRect();
    const width = dock() === 'right' ? window.innerWidth - ev.clientX : ev.clientX - box.left;
    node.style.width = `${Math.max(200, Math.min(width, main.clientWidth - 160))}px`;
  }
  fitActive();
}, () => state.store.set(dock() === 'bottom' ? 'termHeight' : 'termWidth', dock() === 'bottom' ? $('term').offsetHeight : $('term').offsetWidth)));

$('term-tabs').addEventListener('click', e => {
  const tab = e.target.closest('.term-tab');
  if (!tab) return;
  const id = +tab.dataset.id;
  if (e.target.closest('.term-tab-x')) closeTerm(id);
  else activate(id);
});

$('term-tabs').addEventListener('mousedown', e => {
  if (e.button === 1 && e.target.closest('.term-tab')) e.preventDefault();
});

$('term-tabs').addEventListener('auxclick', e => {
  const tab = e.button === 1 && e.target.closest('.term-tab');
  if (tab) closeTerm(+tab.dataset.id);
});

$('term-add').addEventListener('click', () => newTerminal());
$('term-shell').addEventListener('change', e => {
  prefs.set('termShell', e.target.value);
  newTerminal();
});
$('term-close').addEventListener('click', hideTerminal);
