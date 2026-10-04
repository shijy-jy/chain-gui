// 从渲染器侧截图（fromSurface:false）——绕开虚拟桌面合成器无帧的问题
const http = require('http');
const fs = require('fs');
const path = require('path');
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
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

(async () => {
  const out = process.argv[2] || '_shots/3d-renderer.png';
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
  await cdp(ws, 'Page.enable');
  // 1) 渲染器侧截图（不依赖窗口合成器）
  const r = await cdp(ws, 'Page.captureScreenshot', { format: 'png', fromSurface: false, captureBeyondViewport: false });
  fs.mkdirSync(path.dirname(path.resolve(out)), { recursive: true });
  fs.writeFileSync(out, Buffer.from(r.data, 'base64'));
  const size = fs.statSync(out).size;
  console.log(`renderer-side: ${out} ${(size / 1024).toFixed(1)} KB`);
  // 2) 附带读一次视口尺寸，便于判断是否 0×0
  const vp = await cdp(ws, 'Runtime.evaluate', {
    expression: `(() => ({ w: window.innerWidth, h: window.innerHeight, dpr: devicePixelRatio }))()`,
    returnByValue: true,
  });
  console.log('viewport:', JSON.stringify(vp.result.value));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
