// _q_ls.cjs —— 读渲染进程 localStorage 与界面线索（判断布局选择器状态从哪来）
const http = require('http');
const PORT = process.env.CDP_PORT || '9224';
function getJson(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => { let d = ''; res.on('data', (c) => (d += c)); res.on('end', () => resolve(JSON.parse(d))); })
      .on('error', reject);
  });
}
let seq = 0;
const pend = new Map();
function cdp(ws, m, p = {}) {
  return new Promise((res, rej) => { const id = ++seq; pend.set(id, { res, rej }); ws.send(JSON.stringify({ id, method: m, params: p })); });
}
(async () => {
  const t = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  const pg = t.find((x) => x.type === 'page');
  const ws = new WebSocket(pg.webSocketDebuggerUrl);
  ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && pend.has(d.id)) { const p = pend.get(d.id); pend.delete(d.id); d.error ? p.rej(new Error(d.error.message)) : p.res(d.result); } };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = () => j(new Error('ws')); });
  const r = await cdp(ws, 'Runtime.evaluate', {
    expression: `(() => {
      const ls = {};
      for (let i = 0; i < localStorage.length; i++) { const k = localStorage.key(i); ls[k] = localStorage.getItem(k); }
      const sels = Array.from(document.querySelectorAll('select.layout-select')).map((s) => ({ value: s.value, text: s.options[s.selectedIndex] ? s.options[s.selectedIndex].text : null }));
      const labels = Array.from(document.querySelectorAll('.slider-label')).map((x) => x.innerText.replace(/\\s+/g, ' ').slice(0, 24));
      const hasShells = !!document.querySelector('canvas');
      return JSON.stringify({ ls, sels, labels, hasShells, html: document.body.innerHTML.length, hasLayoutWord: document.body.innerHTML.includes('自动（按模式）') });
    })()`,
    returnByValue: true,
  });
  console.log(r.result.value);
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
