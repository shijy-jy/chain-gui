// _q_toolbar.cjs —— 工具栏是否溢出（P2-8 新增「布局」选择器后是否被挤出可视区）
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
      const h = document.querySelector('header.toolbar');
      const sel = Array.from(document.querySelectorAll('select.layout-select'));
      const rect = (el) => { const r = el.getBoundingClientRect(); return { l: Math.round(r.left), r: Math.round(r.right), w: Math.round(r.width) }; };
      return JSON.stringify({
        win: window.innerWidth,
        header: h ? { clientW: h.clientWidth, scrollW: h.scrollWidth, rect: rect(h) } : null,
        selects: sel.map((s) => ({ value: s.value, text: s.options[s.selectedIndex].text, rect: rect(s) })),
        sliders: Array.from(document.querySelectorAll('.slider-label')).map((s) => s.innerText.replace(/\\s+/g, ' ').slice(0, 18) + ' @' + Math.round(s.getBoundingClientRect().left)),
        repai: Array.from(document.querySelectorAll('header.toolbar button')).map((b) => b.innerText.trim() + '@' + Math.round(b.getBoundingClientRect().left) + '-' + Math.round(b.getBoundingClientRect().right)),
      });
    })()`,
    returnByValue: true,
  });
  console.log(r.result.value);
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
