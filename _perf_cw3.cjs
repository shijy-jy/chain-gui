// 诊断 about:blank 新窗口：readyState/location/异常/console
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
  console.log('targets:', targets.map((t) => t.title + ' | ' + t.url.slice(0, 80)).join('\n'));
  const blank = targets.find((t) => t.url === 'about:blank');
  if (!blank) {
    console.log('no blank target');
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
    if (data.method === 'Runtime.consoleAPICalled') {
      events.push('CONSOLE[' + data.params.type + ']: ' + data.params.args.map((a) => a.value ?? a.description ?? '').join(' ').slice(0, 200));
    }
    if (data.method === 'Inspector.targetCrashed') {
      events.push('TARGET CRASHED');
    }
  };
  await new Promise((res) => { ws.onopen = res; });
  await cdp(ws, 'Runtime.enable');
  await cdp(ws, 'Log.enable');
  const r = await cdp(ws, 'Runtime.evaluate', {
    expression: `JSON.stringify({ href: location.href, readyState: document.readyState, title: document.title, bodyLen: document.body ? document.body.innerHTML.length : 0, initScript: typeof window.__CODE_VIEW__ })`,
    returnByValue: true,
  });
  console.log('blank window state:', r.result.value);
  await sleep(3000);
  console.log('events:', events.length ? events.join('\n---\n') : '(none)');
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
