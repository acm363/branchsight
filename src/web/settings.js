import { $, closeLayer, openLayer } from './dom.js';
import * as log from './log.js';
import { prefs } from './prefs.js';
import * as projects from './projects.js';
import { state } from './state.js';

const THEMES = ['system', 'light', 'dark'];
const FONT_SIZES = { min: 10, max: 20, default: 13 };

const fontSize = () => prefs.get('fontSize') ?? FONT_SIZES.default;

export function restore() {
  setTheme(prefs.get('theme'));
  setFontSize(fontSize());
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
  const size = Math.max(FONT_SIZES.min, Math.min(FONT_SIZES.max, px));
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
});

matchMedia('(prefers-color-scheme: dark)').addEventListener('change', log.render);
