// _set_algo.cjs —— 在已安装 app 里临时切「布局」选择器并截图（验完恢复原值）
const http = require('http');
const fs = require('fs');
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
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function ev(ws, expr) {
  const r = await cdp(ws, 'Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  return r.result.value;
}
(async () => {
  const wanted = process.argv[2] || 'auto';
  const out = process.argv[3] || '_shots/installed-auto.png';
  const t = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  const pg = t.find((x) => x.type === 'page');
  const ws = new WebSocket(pg.webSocketDebuggerUrl);
  ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && pend.has(d.id)) { const p = pend.get(d.id); pend.delete(d.id); d.error ? p.rej(new Error(d.error.message)) : p.res(d.result); } };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = () => j(new Error('ws')); });
  await cdp(ws, 'Runtime.enable');
  await cdp(ws, 'Page.enable');
  const before = await ev(ws, `(() => { const s = document.querySelectorAll('select.layout-select')[1]; return s ? s.value : null; })()`);
  const set = await ev(ws, `(() => {
    const sels = Array.from(document.querySelectorAll('select.layout-select'));
    const s = sels.find((x) => Array.from(x.options).some((o) => o.value === 'hierarchy')) || sels[1];
    if (!s) return 'no-select';
    s.value = ${JSON.stringify(wanted)};
    s.dispatchEvent(new Event('change', { bubbles: true }));
    return s.value;
  })()`);
  console.log(`布局选择器：before=${before} -> set=${set}`);
  await sleep(2500);
  const eff = await ev(ws, `(() => { const d = window.__engramDebug; return d.layout.algo + '/' + d.layout.effectiveAlgo + ' shells=' + !!document.querySelector('canvas'); })()`);
  console.log('layout:', eff);
  const shot = await cdp(ws, 'Page.captureScreenshot', { format: 'png', fromSurface: true, captureBeyondViewport: false });
  fs.writeFileSync(out, Buffer.from(shot.data, 'base64'));
  console.log('shot:', out, fs.statSync(out).size, 'bytes');
  // 恢复原值（尊重用户既有偏好）
  if (before) {
    await ev(ws, `(() => { const sels = Array.from(document.querySelectorAll('select.layout-select')); const s = sels.find((x) => Array.from(x.options).some((o) => o.value === 'hierarchy')) || sels[1]; if (s) { s.value = ${JSON.stringify(before)}; s.dispatchEvent(new Event('change', { bubbles: true })); } return true; })()`);
    console.log('已恢复：', before);
  }
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
