// 2.13.1 代码栏滚动功能验证：滚轮事件 → pane.scrollTop 变化
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

  // 打开 engram（开发页签）+ 点击 engram-core-map（86KB 大骨架）
  await evalJs(`(() => {
    const tab = Array.from(document.querySelectorAll('.mode-tab')).find((t) => t.textContent.includes('开发'));
    if (tab && !tab.className.includes('active')) { tab.click(); }
    return 'tab';
  })()`);
  await sleep(800);
  await evalJs(`(() => {
    const row = Array.from(document.querySelectorAll('.ws-row')).find((r) => r.textContent.includes('G:\\\\engram'));
    if (row) { row.click(); return 'clicked'; }
    return 'NO ROW';
  })()`);
  await sleep(6000);

  const pos = await evalJs(`(() => {
    const d = window.__engramDebug;
    const p = d.cy.getElementById('engram-core-map').renderedPosition();
    const wrap = document.querySelector('.canvas-wrap');
    const r = wrap.getBoundingClientRect();
    return JSON.stringify({ x: r.left + p.x, y: r.top + p.y });
  })()`);
  const cp = JSON.parse(pos);
  await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: cp.x, y: cp.y, button: 'left', clickCount: 1 });
  await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: cp.x, y: cp.y, button: 'left', clickCount: 1 });
  await sleep(2500);

  // 面板状态 + 定位（先把侧栏滚到底，代码栏完全进入视口）
  const before = await evalJs(`(() => {
    const sc = document.querySelector('.sidebar-scroll');
    if (sc) sc.scrollTop = sc.scrollHeight;
    const pane = document.querySelector('.code-pane');
    if (!pane) return JSON.stringify({ err: 'NO PANE' });
    const r = pane.getBoundingClientRect();
    return JSON.stringify({
      computedOverflowY: getComputedStyle(pane).overflowY,
      clientH: pane.clientHeight,
      scrollH: pane.scrollHeight,
      scrollTop: pane.scrollTop,
      cx: r.left + r.width / 2,
      cy: r.top + Math.min(r.height / 2, 100),
      winH: window.innerHeight,
      mdOverflow: (() => { const md = document.querySelector('.code-md'); return md ? getComputedStyle(md).overflow : null; })(),
    });
  })()`);
  console.log('BEFORE', before);
  const b = JSON.parse(before);

  // 真实滚轮事件滚 8 次
  for (let i = 0; i < 8; i++) {
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseWheel', x: b.cx, y: b.cy, deltaX: 0, deltaY: 120 });
    await sleep(120);
  }
  const after = await evalJs(`(() => {
    const pane = document.querySelector('.code-pane');
    return JSON.stringify({ scrollTop: pane.scrollTop, scrollH: pane.scrollHeight, clientH: pane.clientHeight });
  })()`);
  console.log('AFTER', after);

  const shot = await cdp(ws, 'Page.captureScreenshot', { format: 'png' });
  fs.writeFileSync('G:/test1.x/_perf/code_scroll.png', Buffer.from(shot.data, 'base64'));
  console.log('screenshot saved');
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
