export const $ = id => document.getElementById(id);

const layers = [];

export function openLayer(layer) {
  layers.filter(other => other !== layer).forEach(closeLayer);
  if (!layers.includes(layer)) layers.push(layer);
}

export function closeLayer(layer) {
  const at = layers.indexOf(layer);
  if (at < 0 || layer.closable?.() === false) return false;
  layers.splice(at, 1);
  layer.close();
  return true;
}

export const topLayer = () => layers.at(-1);

document.addEventListener('mousedown', e => {
  while (layers.length && !topLayer().contains(e.target) && closeLayer(topLayer()));
});

export function drag(event, onMove, onEnd = () => {}) {
  event.preventDefault();
  let frame = 0, last = event;
  const move = e => {
    last = e;
    frame ||= requestAnimationFrame(() => {
      frame = 0;
      onMove(last);
    });
  };
  const up = () => {
    if (frame) {
      cancelAnimationFrame(frame);
      onMove(last);
    }
    window.removeEventListener('mousemove', move);
    window.removeEventListener('mouseup', up);
    onEnd();
  };
  window.addEventListener('mousemove', move);
  window.addEventListener('mouseup', up);
}

let toastTimer = 0;

export function toast(text, ms = 1400) {
  $('toast').textContent = text;
  $('toast').classList.add('on');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => $('toast').classList.remove('on'), ms);
}

export async function copy(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const area = Object.assign(document.createElement('textarea'), { value: text });
    document.body.append(area);
    area.select();
    document.execCommand('copy');
    area.remove();
  }
  toast(`copied ${text}`);
}

export function confirmAction(text, okLabel) {
  const dialog = $('confirm');
  $('confirm-text').textContent = text;
  dialog.querySelector('[value="ok"]').textContent = okLabel;
  dialog.returnValue = '';
  const layer = { contains: () => true, close: () => {}, keydown: () => true };
  openLayer(layer);
  dialog.showModal();
  return new Promise(resolve => dialog.addEventListener('close', () => {
    closeLayer(layer);
    resolve(dialog.returnValue === 'ok');
  }, { once: true }));
}
