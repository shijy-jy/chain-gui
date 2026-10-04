// 直接 invoke 探针 + 轮询新窗口
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
  const mainPage = targets.find((t) => t.url.includes('tauri.localhost'));
  const ws = new WebSocket(mainPage.webSocketDebuggerUrl);
  ws.onmessage = (msg) => {
    const data = JSON.parse(msg.data);
    if (data.id && pending.has(data.id)) {
      const p = pending.get(data.id);
      pending.delete(data.id);
      data.error ? p.reject(new Error(data.error.message)) : p.resolve(data.result);
    }
  };
  await new Promise((res) => { ws.onopen = res; });
  await cdp(ws, 'Runtime.enable');
  const r = await cdp(ws, 'Runtime.evaluate', {
    expression: `window.__TAURI_INTERNALS__.invoke('open_code_window', { dir: 'G:\\\\engram', nodeId: 'engram-core-map' }).then((x) => 'OK ' + JSON.stringify(x)).catch((e) => 'ERR ' + String(e))`,
    awaitPromise: true,
    returnByValue: true,
  });
  console.log('invoke:', r.result.value);
  for (let i = 0; i < 6; i++) {
    await sleep(2000);
    const t2 = await getJson('http://127.0.0.1:9222/json/list');
    console.log(`poll ${i}: ` + t2.map((t) => t.title + ' | ' + t.url.slice(0, 100)).join(' ;; '));
    if (t2.some((t) => t.url.includes('view=code'))) break;
  }
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
