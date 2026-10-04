// 诊断：CDP 目标枚举 + 每个页面里的阅读模式状态
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
  return new Promise((resolve, reject) => { const id = ++seq; pending.set(id, { resolve, reject }); ws.send(JSON.stringify({ id, method, params })); });
}
async function probe(target) {
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && pending.has(d.id)) { const p = pending.get(d.id); pending.delete(d.id); d.error ? p.reject(new Error(d.error.message)) : p.resolve(d.result); }
  };
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws')); });
  await cdp(ws, 'Runtime.enable');
  const r = await cdp(ws, 'Runtime.evaluate', {
    expression: `JSON.stringify({
      hasReadBtn: !!document.querySelector('.read-toggle'),
      readBtnText: document.querySelector('.read-toggle')?.textContent?.trim() || null,
      overlay: !!document.querySelector('.rm-mask'),
      lsViewMode: localStorage.getItem('engram-view-mode'),
      lsLastDir: localStorage.getItem('chain-gui-last-dir'),
      lsReadLast: localStorage.getItem('engram-read-last'),
      root: window.__engramDebug?.snapshot?.manifest?.root ?? null,
      nodes: window.__engramDebug?.snapshot?.nodes?.length ?? -1,
      dir: document.querySelector('.dir')?.textContent?.trim() ?? null,
    })`,
    returnByValue: true,
  });
  ws.close();
  return JSON.parse(r.result.value);
}
(async () => {
  const targets = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  console.log('targets:', targets.map((t) => `${t.type} | ${t.title} | ${t.url}`).join('\n         '));
  for (const t of targets.filter((x) => x.type === 'page')) {
    try {
      console.log('\n--- ' + t.url + ' (' + t.id + ') ---');
      console.log(JSON.stringify(await probe(t), null, 2));
    } catch (e) { console.log('probe failed: ' + e.message); }
  }
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
