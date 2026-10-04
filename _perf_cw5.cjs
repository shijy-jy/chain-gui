const http = require('http');
function getJson(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => {
      let data = '';
      res.on('data', (c) => (data += c));
      res.on('end', () => resolve(JSON.parse(data)));
    }).on('error', reject);
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
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
(async () => {
  const targets = await getJson('http://127.0.0.1:9222/json/list');
  const blank = targets.find((t) => t.url === 'about:blank');
  if (!blank) {
    console.log('no blank target; targets:', targets.map((t) => t.url).join(', '));
    process.exit(0);
  }
  const ws = new WebSocket(blank.webSocketDebuggerUrl);
  const events = [];
  ws.onmessage = (msg) => {
    const data = JSON.parse(msg.data);
    if (data.id && pending.has(data.id)) {
      const p = pending.get(data.id);
      pending.delete(data.id);
      data.error ? p.reject(new Error(data.error.message)) : p.resolve(data.result);
      return;
    }
    if (data.method === 'Runtime.exceptionThrown') {
      events.push('EXC: ' + (data.params.exceptionDetails.exception?.description ?? data.params.exceptionDetails.text));
    }
    if (data.method === 'Inspector.targetCrashed') events.push('CRASHED');
    if (data.method === 'Page.frameNavigated') events.push('NAV: ' + JSON.stringify(data.params.frame.url));
    if (data.method === 'Page.loadEventFired') events.push('LOADED');
  };
  await new Promise((res) => { ws.onopen = res; });
  await cdp(ws, 'Runtime.enable');
  await cdp(ws, 'Page.enable');
  const r = await cdp(ws, 'Runtime.evaluate', {
    expression: `location.href = 'http://tauri.localhost/index.html'; 'navigating'`,
    returnByValue: true,
  });
  console.log('nav eval:', r.result.value);
  await sleep(6000);
  const state = await cdp(ws, 'Runtime.evaluate', {
    expression: `JSON.stringify({ href: location.href, readyState: document.readyState, title: document.title, bodyLen: document.body ? document.body.innerHTML.length : 0 })`,
    returnByValue: true,
  });
  console.log('after nav:', state.result.value);
  console.log('events:', events.join(' ;; '));
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
