// 测 perf1500 当前布局的连线交叉数（基线）
const http = require('http');
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
  // 打开 perf1500（开发页签）
  await evalJs(`(() => {
    const tab = Array.from(document.querySelectorAll('.mode-tab')).find((t) => t.textContent.includes('开发'));
    if (tab && !tab.className.includes('active')) { tab.click(); return 'tab'; }
    return 'already';
  })()`);
  await sleep(800);
  await evalJs(`(() => {
    const row = Array.from(document.querySelectorAll('.ws-row')).find((r) => r.textContent.includes('perf1500'));
    if (row) { row.click(); return 'clicked'; }
    return 'NO ROW';
  })()`);
  await sleep(12000); // 等布局收敛

  const result = await evalJs(`(() => {
    const d = window.__engramDebug;
    const cy = d.cy;
    const nodes = cy.nodes().toArray();
    const pos = new Map();
    nodes.forEach((n) => pos.set(n.id(), n.position()));
    const segCross = (p1, p2, p3, p4) => {
      const cross = (o, a, b) => (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
      const d1 = cross(p3, p4, p1), d2 = cross(p3, p4, p2), d3 = cross(p1, p2, p3), d4 = cross(p1, p2, p4);
      return ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0));
    };
    const edges = cy.edges().map((e) => ({ a: e.source().id(), b: e.target().id() }));
    let c = 0;
    for (let i = 0; i < edges.length; i++) {
      for (let j = i + 1; j < edges.length; j++) {
        const e1 = edges[i], e2 = edges[j];
        if (e1.a === e2.a || e1.a === e2.b || e1.b === e2.a || e1.b === e2.b) continue;
        if (segCross(pos.get(e1.a), pos.get(e1.b), pos.get(e2.a), pos.get(e2.b))) c++;
      }
    }
    return JSON.stringify({ nodes: nodes.length, edges: edges.length, crossings: c });
  })()`);
  console.log('BASELINE', result);
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
