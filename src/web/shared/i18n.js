import { prefs } from './prefs.js';
import { en } from '../i18n/en.js';
import { fr } from '../i18n/fr.js';

const CATALOGS = { en, fr };
export const LOCALES = ['en', 'fr'];

function detect() {
  const saved = prefs.get('lang');
  if (LOCALES.includes(saved)) return saved;
  return (navigator.language || '').toLowerCase().startsWith('fr') ? 'fr' : 'en';
}

let locale = detect();
document.documentElement.lang = locale;

const listeners = new Set();
export const onLocaleChange = cb => { listeners.add(cb); };

export const getLocale = () => locale;

export function setLocale(next) {
  if (!LOCALES.includes(next) || next === locale) return;
  locale = next;
  prefs.set('lang', locale);
  document.documentElement.lang = locale;
  listeners.forEach(cb => cb());
}

const strings = () => CATALOGS[locale] || en;

export function t(key, params) {
  const s = strings().strings[key] ?? en.strings[key] ?? key;
  return params ? s.replace(/\{(\w+)\}/g, (_, k) => (params[k] ?? `{${k}}`)) : s;
}

export function plural(n, noun) {
  const forms = strings().nouns[noun] || en.nouns[noun] || { one: noun, other: `${noun}s` };
  const word = forms[new Intl.PluralRules(locale).select(n)] || forms.other;
  return `${n.toLocaleString(locale)} ${word}`;
}

export function applyStatic(root = document) {
  root.querySelectorAll('[data-i18n]').forEach(el => { el.textContent = t(el.dataset.i18n); });
  root.querySelectorAll('[data-i18n-title]').forEach(el => { el.title = t(el.dataset.i18nTitle); });
  root.querySelectorAll('[data-i18n-ph]').forEach(el => { el.placeholder = t(el.dataset.i18nPh); });
}
