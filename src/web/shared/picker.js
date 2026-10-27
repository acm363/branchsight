import { closeLayer, openLayer } from './dom.js';
import { clamp } from './format.js';

export function picker({ anchor, panel, input, list, items, row, choose, empty, tabChooses = false, modal = () => false }) {
  let shown = [], cursor = 0;
  const layer = {
    contains: target => modal() || anchor.contains(target),
    closable: () => !modal(),
    close: () => {
      if (panel.contains(document.activeElement)) document.activeElement.blur();
      panel.hidden = true;
    },
    keydown(e) {
      if (e.target !== input && e.target !== document.body) return e.key !== 'Escape';
      const move = { ArrowDown: 1, ArrowUp: -1 }[e.key];
      if (move) {
        cursor = clamp(cursor + move, 0, shown.length - 1);
        render();
      } else if (e.key === 'Enter' || (tabChooses && e.key === 'Tab')) {
        if (shown[cursor]) choose(shown[cursor]);
      } else return false;
      e.preventDefault();
      return true;
    },
  };

  function render() {
    shown = items(input.value.trim());
    cursor = Math.min(cursor, Math.max(0, shown.length - 1));
    list.innerHTML = shown.map((item, i) => row(item, i, i === cursor)).join('') || `<div class="pick"><span class="name">${empty()}</span></div>`;
    list.querySelector('.cur')?.scrollIntoView({ block: 'nearest' });
  }

  input.addEventListener('input', () => {
    cursor = 0;
    render();
  });

  list.addEventListener('click', e => {
    const el = e.target.closest('[data-i]');
    if (!el) return;
    cursor = +el.dataset.i;
    choose(shown[cursor]);
    input.focus();
  });

  return {
    get isOpen() { return !panel.hidden; },
    render,
    open() {
      panel.hidden = false;
      input.value = '';
      cursor = 0;
      openLayer(layer);
      render();
      input.focus();
    },
    close: () => closeLayer(layer),
  };
}
