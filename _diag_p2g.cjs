// CDP 验证第 5 段：信息栏完整内容 dump（找"过程日志"按钮所在的容器，打印其文本与子元素）
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
async function evalJs(ws, expr) {
  const r = await cdp(ws, 'Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  return r.result.value;
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

  const dump = await evalJs(ws, `(() => {
    const btn = [...document.querySelectorAll('button')].find(b => b.textContent.includes('过程日志'));
    if (!btn) return { found: false };
    let el = btn;
    while (el.parentElement && el.parentElement.children.length < 40 && !el.parentElement.querySelector('.cy-container')) {
      el = el.parentElement;
    }
    const btns = [...el.querySelectorAll('button')].map(b => b.textContent.trim()).filter(t => t.length > 0);
    const text = el.innerText.slice(0, 1200);
    return { found: true, buttons: btns, text: text };
  })()`);
  console.log('sidebar-dump:', JSON.stringify(dump, null, 1));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
