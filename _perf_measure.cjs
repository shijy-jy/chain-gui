// 性能基线测量：CDP 驱动 GUI 打开 perf1500，测 空闲/缩放/平移/涟漪 FPS + 截图
const http = require('http');
const fs = require('fs');
function getJson(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => {
      let data = '';
      res.on('data', (c) => (data += c));
      res.on('end', () => resolve(JSON.parse(data)));
    }).on('error', reject);
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
  const targets = await getJson('http://127.0.0.1:9222/json/list');
  const page = targets.find((t) => t.type === 'page');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  ws.onmessage = (msg) => {
    const data = JSON.parse(msg.data);
    if (data.id && pending.has(data.id)) {
      const p = pending.get(data.id);
      pending.delete(data.id);
      data.error ? p.reject(new Error(data.error.message)) : p.resolve(data.result);
    }
  };
  await new Promise((res) => { ws.onopen = res; });
  await cdp(ws, 'Runtime.enable');

  const evalJs = async (expression, awaitPromise = false) => {
    const r = await cdp(ws, 'Runtime.evaluate', { expression, awaitPromise, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.text + ' ' + (r.exceptionDetails.exception?.description ?? ''));
    return r.result.value;
  };

  // 1. 点击工作区列表里的 perf1500
  const clicked = await evalJs(`(() => {
    const rows = Array.from(document.querySelectorAll('.ws-row'));
    const row = rows.find((r) => r.textContent.includes('perf1500'));
    if (!row) return 'NO ROW';
    row.click();
    return 'clicked';
  })()`);
  console.log('workspace click:', clicked);
  if (clicked === 'NO ROW') {
    // 兜底：直接 invoke scan 并重载（若 app 有 loadWorkspace 类入口）
    console.log('fallback: invoke scan via internals');
    await evalJs(`window.__TAURI_INTERNALS__.invoke('scan_chain', { dir: 'G:\\\\perf1500', mode: 'dev' }).then(s => 'n=' + s.nodes.length)`, true);
  }
  await sleep(10000); // 等扫描+布局+fit

  // 2. FPS 采样器（页面内 rAF 计时）
  const sampler = `(durMs) => new Promise((res) => {
    const deltas = [];
    let last = performance.now();
    let count = 0;
    const start = last;
    function tick(now) {
      deltas.push(now - last); last = now; count++;
      if (now - start < durMs) requestAnimationFrame(tick);
      else {
        deltas.sort((a, b) => a - b);
        const avg = deltas.reduce((s, d) => s + d, 0) / deltas.length;
        res(JSON.stringify({
          frames: count,
          avgMs: +avg.toFixed(2),
          p50: +deltas[Math.floor(deltas.length * 0.5)].toFixed(2),
          p95: +deltas[Math.floor(deltas.length * 0.95)].toFixed(2),
          max: +deltas[deltas.length - 1].toFixed(2),
          fps: +(1000 / avg).toFixed(1),
        }));
      }
    }
    requestAnimationFrame(tick);
  })`;

  // 画布中心坐标
  const center = await evalJs(`(() => {
    const el = document.querySelector('.canvas-wrap') || document.querySelector('.cy-container') || document.body;
    const r = el.getBoundingClientRect();
    return JSON.stringify({ x: r.left + r.width / 2, y: r.top + r.height / 2, w: r.width, h: r.height });
  })()`);
  const c = JSON.parse(center);
  console.log('canvas center:', center);

  const results = {};
  results.idle = JSON.parse(await evalJs(`(${sampler})(3000)`, true));

  // 3. 缩放（滚轮×2 每 400ms 一次，采样 3s）
  (async () => {
    for (let i = 0; i < 6; i++) {
      await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseWheel', x: c.x, y: c.y, deltaX: 0, deltaY: i % 2 ? 120 : -120 });
      await sleep(400);
    }
  })();
  results.zoom = JSON.parse(await evalJs(`(${sampler})(3000)`, true));

  // 4. 平移（拖拽两段）
  (async () => {
    for (const [dx, dy] of [[-300, -100], [250, 150], [-150, 200]]) {
      await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: c.x, y: c.y, button: 'left', buttons: 1 });
      await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseMoved', x: c.x + dx, y: c.y + dy, button: 'left', buttons: 1 });
      await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: c.x + dx, y: c.y + dy, button: 'left' });
      await sleep(500);
    }
  })();
  results.pan = JSON.parse(await evalJs(`(${sampler})(3000)`, true));

  // 5. 点击画布中心（根节点）→ 涟漪激活，采样 5s
  await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: c.x, y: c.y, button: 'left', clickCount: 1 });
  await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: c.x, y: c.y, button: 'left', clickCount: 1 });
  await sleep(1500);
  results.ripple = JSON.parse(await evalJs(`(${sampler})(5000)`, true));

  // 6. 截图
  const shot = await cdp(ws, 'Page.captureScreenshot', { format: 'png' });
  fs.writeFileSync('G:/test1.x/_perf/base_perf1500.png', Buffer.from(shot.data, 'base64'));
  console.log('screenshot saved');

  console.log('RESULTS ' + JSON.stringify(results, null, 1));
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
