// 复测：利用 __engramDebug 钩子精确测 2.11.0（节点数校验 + 根节点点击 + FPS 采样 + 截图）
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

  // 0. 打开 perf1500（若未打开）
  const cur = await evalJs(`document.body.innerText.includes('G:\\perf1500') ? 'already' : 'need'`);
  if (cur === 'need') {
    await evalJs(`(() => {
      const row = Array.from(document.querySelectorAll('.ws-row')).find((r) => r.textContent.includes('perf1500'));
      if (row) { row.click(); return 'clicked'; }
      return 'NO ROW';
    })()`);
    await sleep(10000);
  }
  // 0.5 校验节点数（新钩子）
  const state = await evalJs(`(() => {
    const d = window.__engramDebug;
    if (!d) return JSON.stringify({ hook: false });
    const cy = d.cy;
    const n0001 = cy.getElementById('n0001');
    return JSON.stringify({
      hook: true,
      nodes: d.snapshot ? d.snapshot.nodes.length : -1,
      cyNodes: cy.nodes().length,
      cyEdges: cy.edges().length,
      rootPos: n0001.nonempty() ? n0001.renderedPosition() : null,
      rootVisible: cy.elements().boundingBox(),
    });
  })()`);
  console.log('STATE', state);
  const st = JSON.parse(state);

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
          frames: count, avgMs: +avg.toFixed(2),
          p50: +deltas[Math.floor(deltas.length * 0.5)].toFixed(2),
          p95: +deltas[Math.floor(deltas.length * 0.95)].toFixed(2),
          max: +deltas[deltas.length - 1].toFixed(2),
          fps: +(1000 / avg).toFixed(1),
        }));
      }
    }
    requestAnimationFrame(tick);
  })`;

  const results = {};
  results.idle = JSON.parse(await evalJs(`(${sampler})(3000)`, true));

  // 画布中心（zoom/pan 用）
  const center = await evalJs(`(() => {
    const el = document.querySelector('.canvas-wrap') || document.body;
    const r = el.getBoundingClientRect();
    return JSON.stringify({ x: r.left + r.width / 2, y: r.top + r.height / 2 });
  })()`);
  const c = JSON.parse(center);

  (async () => {
    for (let i = 0; i < 6; i++) {
      await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseWheel', x: c.x, y: c.y, deltaX: 0, deltaY: i % 2 ? 120 : -120 });
      await sleep(400);
    }
  })();
  results.zoom = JSON.parse(await evalJs(`(${sampler})(3000)`, true));

  (async () => {
    for (const [dx, dy] of [[-300, -100], [250, 150], [-150, 200]]) {
      await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: c.x, y: c.y, button: 'left', buttons: 1 });
      await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseMoved', x: c.x + dx, y: c.y + dy, button: 'left', buttons: 1 });
      await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: c.x + dx, y: c.y + dy, button: 'left' });
      await sleep(500);
    }
  })();
  results.pan = JSON.parse(await evalJs(`(${sampler})(3000)`, true));

  // 精确点击根节点（钩子拿到 renderedPosition → 转视口坐标）
  const clickPos = await evalJs(`(() => {
    const d = window.__engramDebug;
    const p = d.cy.getElementById('n0001').renderedPosition();
    const wrap = document.querySelector('.canvas-wrap');
    const wr = wrap.getBoundingClientRect();
    return JSON.stringify({ x: wr.left + p.x, y: wr.top + p.y });
  })()`);
  const cp = JSON.parse(clickPos);
  await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: cp.x, y: cp.y, button: 'left', clickCount: 1 });
  await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: cp.x, y: cp.y, button: 'left', clickCount: 1 });
  await sleep(1500);
  results.ripple = JSON.parse(await evalJs(`(${sampler})(5000)`, true));

  // 涟漪状态校验
  const ripState = await evalJs(`(() => {
    const d = window.__engramDebug;
    const cy = d.cy;
    let ripNodes = 0;
    cy.nodes().forEach((n) => { if (n.classes().some((cl) => cl.startsWith('rip'))) ripNodes++; });
    return JSON.stringify({ ripNodes, hasSource: true });
  })()`);
  console.log('RIPSTATE', ripState);

  const shot = await cdp(ws, 'Page.captureScreenshot', { format: 'png' });
  fs.writeFileSync('G:/test1.x/_perf/after_perf1500.png', Buffer.from(shot.data, 'base64'));
  console.log('screenshot saved');
  console.log('RESULTS ' + JSON.stringify(results, null, 1));
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
