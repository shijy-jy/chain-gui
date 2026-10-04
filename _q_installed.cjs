// _q_installed.cjs —— 查询已安装 app（CDP_PORT 可指定）并截图
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
(async () => {
  const t = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  const pg = t.find((x) => x.type === 'page');
  const ws = new WebSocket(pg.webSocketDebuggerUrl);
  ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && pend.has(d.id)) { const p = pend.get(d.id); pend.delete(d.id); d.error ? p.rej(new Error(d.error.message)) : p.res(d.result); } };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = () => j(new Error('ws')); });
  await cdp(ws, 'Page.enable');
  await cdp(ws, 'Runtime.enable');
  await cdp(ws, 'Page.bringToFront');
  await sleep(2000);
  const st = await cdp(ws, 'Runtime.evaluate', {
    expression: `(() => { const d = window.__engramDebug; return JSON.stringify({
      url: location.href,
      mode: d ? d.mode : null,
      algo: d ? d.layout.algo : null,
      eff: d ? d.layout.effectiveAlgo : null,
      nodes: d && d.snapshot ? d.snapshot.nodes.length : 0,
      canvas: !!document.querySelector('canvas'),
      legendFolded: !!document.querySelector('.legend.folded'),
      bodyLen: document.body.innerText.length,
    }); })()`,
    returnByValue: true,
  });
  console.log('state:', st.result.value);
  await sleep(1200);
  const shot = await cdp(ws, 'Page.captureScreenshot', { format: 'png', fromSurface: false, captureBeyondViewport: false });
  const out = process.argv[2] || '_shots/installed.png';
  fs.writeFileSync(out, Buffer.from(shot.data, 'base64'));
  console.log('shot:', out, fs.statSync(out).size, 'bytes');
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
