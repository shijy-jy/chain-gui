// CDP 诊断2：重载页面并捕获控制台错误/异常，5 秒后看挂载状态
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
const events = [];
function cdp(ws, method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++seq;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
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
      return;
    }
    if (d.method === 'Runtime.exceptionThrown') {
      events.push('EXCEPTION: ' + (d.params.exceptionDetails.exception?.description || d.params.exceptionDetails.text || JSON.stringify(d.params).slice(0, 300)));
    }
    if (d.method === 'Runtime.consoleAPICalled' && d.params.type === 'error') {
      events.push('CONSOLE.ERROR: ' + d.params.args.map((a) => a.value ?? a.description ?? '').join(' ').slice(0, 400));
    }
  };
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws')); });
  await cdp(ws, 'Runtime.enable');
  await cdp(ws, 'Page.enable');
  await cdp(ws, 'Page.reload', { ignoreCache: true });
  await sleep(6000);
  const r = await cdp(ws, 'Runtime.evaluate', {
    expression: `(() => ({
      readyState: document.readyState,
      bodyLen: document.body ? document.body.innerHTML.length : -1,
      debug: window.__engramDebug ? Object.keys(window.__engramDebug) : null,
      buttons: [...document.querySelectorAll('header button')].map(b => b.textContent.trim()),
      textSample: document.body.innerText.slice(0, 200),
    }))()`,
    returnByValue: true,
  });
  console.log('STATE:', JSON.stringify(r.result.value, null, 1));
  console.log('EVENTS:', events.length ? events.join('\n') : '(none)');
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
