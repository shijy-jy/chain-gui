// CDP 诊断：应用是否挂载、是否有 JS 错误
const http = require('http');
const PORT = '9223';
function getJson(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => { let d = ''; res.on('data', (c) => (d += c)); res.on('end', () => resolve(JSON.parse(d))); })
      .on('error', reject);
  });
}
let seq = 0;
const pending = new Map();
function cdp(ws, method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++seq;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
}
(async () => {
  const targets = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  const page = targets.find((t) => t.type === 'page');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && pending.has(d.id)) {
      const p = pending.get(d.id);
      pending.delete(d.id);
      d.error ? p.reject(new Error(d.error.message)) : p.resolve(d.result);
    }
  };
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws')); });
  await cdp(ws, 'Runtime.enable');
  const r = await cdp(ws, 'Runtime.evaluate', {
    expression: `(() => ({
      readyState: document.readyState,
      bodyLen: document.body ? document.body.innerHTML.length : -1,
      hasApp: !!document.querySelector('#app, .app, main, .cy-container'),
      debug: window.__engramDebug ? Object.keys(window.__engramDebug) : null,
      svelteErr: window.__svelte_error || null,
      errText: (document.querySelector('.error-bar, vite-error-overlay') || {}).textContent || null,
    }))()`,
    returnByValue: true,
  });
  console.log(JSON.stringify(r.result.value, null, 1));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
