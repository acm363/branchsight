import { $, closeLayer, openLayer } from '../shared/dom.js';
import { clamp } from '../shared/format.js';
import { applyStatic, getLocale, setLocale } from '../shared/i18n.js';
import * as log from '../history/list.js';
import * as panel from '../changes/panel.js';
import { prefs } from '../shared/prefs.js';
import * as projects from './projects.js';
import { state } from '../shared/state.js';

const THEMES = ['system', 'light', 'dark'];
const DOCKS = ['right', 'bottom'];
const TERM_DOCKS = ['bottom', 'right', 'left'];
const FONT_SIZES = { min: 10, max: 20, default: 13 };

const fontSize = () => prefs.get('fontSize') ?? FONT_SIZES.default;

export function restore() {
  applyStatic();
  markLang();
  setTheme(prefs.get('theme'));
  setFontSize(fontSize());
  markDock();
  markTermDock();
  panel.applyDock();
}

const dock = () => (DOCKS.includes(prefs.get('dock')) ? prefs.get('dock') : 'right');
const markDock = () => $('dock-seg').querySelectorAll('button').forEach(b => b.classList.toggle('on', b.dataset.dock === dock()));

function setDock(choice) {
  prefs.set('dock', DOCKS.includes(choice) ? choice : 'right');
  markDock();
  panel.applyDock();
}

const termDock = () => (TERM_DOCKS.includes(prefs.get('termDock')) ? prefs.get('termDock') : 'bottom');
const markTermDock = () => $('term-seg')?.querySelectorAll('button').forEach(b => b.classList.toggle('on', b.dataset.termDock === termDock()));

function setTermDock(choice) {
  prefs.set('termDock', TERM_DOCKS.includes(choice) ? choice : 'bottom');
  markTermDock();
  import('../terminal/panel.js').then(m => m.applyTermDock()).catch(() => {});
}

const markLang = () => $('lang-seg').querySelectorAll('button').forEach(b => b.classList.toggle('on', b.dataset.lang === getLocale()));

function setLang(lang) {
  setLocale(lang);
  markLang();
  applyStatic();
  log.render();
}

function setTheme(theme) {
  const choice = THEMES.includes(theme) ? theme : 'system';
  if (choice === 'system') document.documentElement.removeAttribute('data-theme');
  else document.documentElement.dataset.theme = choice;
  $('theme-seg').querySelectorAll('button').forEach(b => b.classList.toggle('on', b.dataset.theme === choice));
  prefs.set('theme', choice);
  log.render();
}

function setFontSize(px) {
  const size = clamp(px, FONT_SIZES.min, FONT_SIZES.max);
  document.documentElement.style.setProperty('--fs', `${size}px`);
  $('fs-val').textContent = `${size}px`;
  prefs.set('fontSize', size);
  log.setRowHeight(Math.round(size * 1.6));
}

const menu = $('mainmenu').querySelector('.mb');

const layer = {
  contains: target => menu.contains(target),
  close: () => {
    menu.classList.remove('open');
    menu.querySelector('.mb-menu').hidden = true;
  },
};

function openMenu() {
  menu.classList.add('open');
  menu.querySelector('.mb-menu').hidden = false;
  menu.querySelector('[data-cmd="close"]').disabled = !state.repo.path;
  openLayer(layer);
}

const commands = {
  open: () => {
    closeLayer(layer);
    projects.open();
  },
  close: () => {
    closeLayer(layer);
    projects.closeProject();
  },
  smaller: () => setFontSize(fontSize() - 1),
  bigger: () => setFontSize(fontSize() + 1),
  'reset-size': () => setFontSize(FONT_SIZES.default),
};

$('mainmenu').addEventListener('click', e => {
  if (e.target.closest('.mb-btn')) return menu.classList.contains('open') ? closeLayer(layer) : openMenu();
  commands[e.target.closest('[data-cmd]')?.dataset.cmd]?.();
  const theme = e.target.closest('#theme-seg [data-theme]')?.dataset.theme;
  if (theme) setTheme(theme);
  const lang = e.target.closest('#lang-seg [data-lang]')?.dataset.lang;
  if (lang) setLang(lang);
  const docked = e.target.closest('#dock-seg [data-dock]')?.dataset.dock;
  if (docked) setDock(docked);
  const termDocked = e.target.closest('#term-seg [data-term-dock]')?.dataset.termDock;
  if (termDocked) setTermDock(termDocked);
});

matchMedia('(prefers-color-scheme: dark)').addEventListener('change', log.render);
