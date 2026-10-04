// _q_insets.cjs —— 直接问渲染进程：insets 的真实值 + 浮层矩形 + 图例折叠态
const http = require('http');
const PORT = process.env.CDP_PORT || '9223';
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
  const r = await cdp(ws, 'Runtime.evaluate', {
    expression: `(() => {
      const d = window.__engramDebug;
      const rect = (sel) => { const el = document.querySelector(sel); if (!el) return null; const r = el.getBoundingClientRect(); return { w: Math.round(r.width), h: Math.round(r.height), l: Math.round(r.left), t: Math.round(r.top), b: Math.round(r.bottom) }; };
      return JSON.stringify({
        insets: d.layout.insets,
        insetsKeys: Object.keys(d.layout.insets || {}),
        insetsJson: JSON.stringify(d.layout.insets),
        legendRect: rect('.legend'),
        zoomRect: rect('.zoom-controls'),
        searchRect: rect('.node-search'),
        canvasRect: rect('canvas'),
        legendFoldedClass: !!document.querySelector('.legend.folded'),
        legendTitle: (document.querySelector('.legend-title') || {}).innerText,
      });
    })()`,
    returnByValue: true,
  });
  console.log(r.result.value);
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
