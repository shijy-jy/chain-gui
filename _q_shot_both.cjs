// _q_shot_both.cjs —— 已安装 app：控制台异常 + 两种截图方式各试一次
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
  const errs = [];
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && pend.has(d.id)) { const p = pend.get(d.id); pend.delete(d.id); d.error ? p.rej(new Error(d.error.message)) : p.res(d.result); return; }
    if (d.method === 'Runtime.exceptionThrown') errs.push((d.params.exceptionDetails.exception?.description || d.params.exceptionDetails.text || '').slice(0, 200));
    if (d.method === 'Runtime.consoleAPICalled' && d.params.type === 'error') errs.push('console.error: ' + JSON.stringify(d.params.args.map((a) => a.value || a.description)).slice(0, 200));
  };
  await new Promise((r, j) => { ws.onopen = r; ws.onerror = () => j(new Error('ws')); });
  await cdp(ws, 'Runtime.enable');
  await cdp(ws, 'Page.enable');
  await cdp(ws, 'Page.bringToFront');
  await sleep(1500);
  for (const [name, fromSurface] of [['renderer', false], ['surface', true]]) {
    try {
      const shot = await cdp(ws, 'Page.captureScreenshot', { format: 'png', fromSurface, captureBeyondViewport: false });
      const out = `_shots/installed-${name}.png`;
      fs.writeFileSync(out, Buffer.from(shot.data, 'base64'));
      console.log(`${name} (fromSurface=${fromSurface}): ${out} ${fs.statSync(out).size} bytes`);
    } catch (e) {
      console.log(`${name} 截图失败：${e.message}`);
    }
  }
  console.log('控制台异常：', errs.length ? JSON.stringify(errs, null, 1) : '无');
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
