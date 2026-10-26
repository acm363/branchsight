import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const [binary, root, outDir] = process.argv.slice(2).map(p => p && resolve(p));
if (!binary || !root || !outDir) {
  console.error('usage: node scripts/screenshots.mjs <branchsight binary> <demo root> <output dir>');
  process.exit(2);
}

const PORT = 7499, CDP_PORT = 9333, WIDTH = 1440, HEIGHT = 700;
const repo = join(root, 'aurora-notes');
const page = (query = '') => `http://127.0.0.1:${PORT}/?${query}`;
const opened = `repo=${encodeURIComponent(repo)}&ref=%40local&ref=%40remotes`;
const sleep = ms => new Promise(r => setTimeout(r, ms));

const row = text => `[...document.querySelectorAll('.row')].find(r => r.querySelector('.t')?.textContent === ${JSON.stringify(text)})`;
const SCENES = [
  { name: 'graph', url: page(opened), run: '' },
  { name: 'commit', url: page(opened), run: `${row("Merge branch 'feature/search'")}.querySelector('.t').click()` },
  { name: 'uncommitted', url: page(opened), run: `document.querySelector('.row.wt .t').click()` },
  {
    name: 'compare',
    url: page(opened),
    run: `${row('chore: update dependencies')}.querySelector('.t').click();
      await new Promise(r => setTimeout(r, 400));
      ${row('feat(search): full-text index')}.querySelector('.t').dispatchEvent(new MouseEvent('click', { bubbles: true, ctrlKey: true, detail: 1 }))`,
  },
  {
    name: 'branch-menu',
    url: page(opened),
    run: `${row('feat: dark theme')}.querySelector('.t').click();
      await new Promise(r => setTimeout(r, 300));
      document.getElementById('close').click();
      const label = [...document.querySelectorAll('.row .ref')].find(e => e.dataset.name === 'feature/tags');
      const r = label.getBoundingClientRect();
      label.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.x + 30, clientY: r.y + r.height }))`,
  },
  { name: 'start', url: page('start'), height: 420, run: '' },
  { name: 'light', url: page(opened), theme: 'light', run: `${row('feat(sync): retry with backoff')}.querySelector('.t').click()` },
];

async function waitFor(fn, what, tries = 100) {
  for (let i = 0; i < tries; i++) {
    try {
      const value = await fn();
      if (value) return value;
    } catch {}
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${what}`);
}

const server = spawn(binary, ['serve', '--port', String(PORT), '--root', root, root], { stdio: 'ignore' });
const profile = mkdtempSync(join(tmpdir(), 'branchsight-shots-'));
const chrome = spawn('chromium-browser', [
  '--headless=new', '--disable-gpu', '--hide-scrollbars', '--no-first-run',
  `--remote-debugging-port=${CDP_PORT}`, `--user-data-dir=${profile}`, `--window-size=${WIDTH},${HEIGHT}`, 'about:blank',
], { stdio: 'ignore' });

try {
  await waitFor(() => fetch(page('')).then(r => r.ok), 'branchsight serve');
  const target = await waitFor(async () => (await (await fetch(`http://127.0.0.1:${CDP_PORT}/json/list`)).json()).find(t => t.type === 'page'), 'chromium');
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((ok, ko) => { ws.onopen = ok; ws.onerror = ko; });
  let id = 0;
  const pending = new Map();
  ws.onmessage = ({ data }) => {
    const msg = JSON.parse(data);
    if (msg.id && pending.has(msg.id)) {
      const { ok, ko } = pending.get(msg.id);
      pending.delete(msg.id);
      msg.error ? ko(new Error(msg.error.message)) : ok(msg.result);
    }
  };
  const send = (method, params = {}) => new Promise((ok, ko) => {
    pending.set(++id, { ok, ko });
    ws.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async expression => {
    const { result, exceptionDetails } = await send('Runtime.evaluate', { expression: `(async () => { ${expression} })()`, awaitPromise: true, returnByValue: true });
    if (exceptionDetails) throw new Error(exceptionDetails.exception?.description || exceptionDetails.text);
    return result.value;
  };

  for (const scene of SCENES) {
    await send('Emulation.setDeviceMetricsOverride', { width: WIDTH, height: scene.height || HEIGHT, deviceScaleFactor: 2, mobile: false });
    await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: scene.theme || 'dark' }] });
    await send('Page.navigate', { url: scene.url });
    await waitFor(() => evaluate(`return document.querySelector(${JSON.stringify(scene.url.includes('start') ? '#plist .ref' : '.row')}) !== null`), `${scene.name} to load`);
    await sleep(500);
    if (scene.run) await evaluate(scene.run);
    await sleep(900);
    const { data } = await send('Page.captureScreenshot', { format: 'png' });
    const file = join(outDir, `${scene.name}.png`);
    writeFileSync(file, Buffer.from(data, 'base64'));
    console.log(`wrote ${file}`);
  }
  ws.close();
} finally {
  chrome.kill();
  server.kill();
  await sleep(300);
  rmSync(profile, { recursive: true, force: true });
}
