// 2.14.0 全屏代码页验证：点 ⧉ → 覆盖层出现 + 内容渲染 + 截图
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
  const mainPage = targets.find((t) => t.url.includes('tauri.localhost'));
  const ws = new WebSocket(mainPage.webSocketDebuggerUrl);
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
  const pos = JSON.parse(await evalJs(`(() => {
    const d = window.__engramDebug;
    const p = d.cy.getElementById('engram-core-map').renderedPosition();
    const wrap = document.querySelector('.canvas-wrap');
    const r = wrap.getBoundingClientRect();
    return JSON.stringify({ x: r.left + p.x, y: r.top + p.y });
  })()`));
  await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: pos.x, y: pos.y, button: 'left', clickCount: 1 });
  await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: pos.x, y: pos.y, button: 'left', clickCount: 1 });
  await sleep(2500);

  // 点击 ⧉
  const clickedBtn = await evalJs(`(() => {
    const btns = Array.from(document.querySelectorAll('.code-max-btn'));
    const btn = btns.find((b) => b.textContent.includes('⧉'));
    if (btn) { btn.click(); return 'clicked'; }
    return 'NO BTN';
  })()`);
  console.log('⧉ click:', clickedBtn);
  await sleep(3500);

  const state = await evalJs(`(() => {
    const mask = document.querySelector('.code-fs-mask');
    if (!mask) return JSON.stringify({ err: 'NO MASK' });
    const t = mask.innerText;
    return JSON.stringify({
      maskVisible: getComputedStyle(mask).display !== 'none',
      hasSkeleton: t.includes('代码骨架'),
      hasNode: t.includes('engram-core-map'),
      hasMermaid: !!mask.querySelector('.cv-mermaid'),
      contentLen: t.length,
      scrollable: mask.scrollHeight > window.innerHeight,
      closeBtn: !!document.querySelector('.code-fs-close'),
    });
  })()`);
  console.log('OVERLAY STATE', state);

  const shot = await cdp(ws, 'Page.captureScreenshot', { format: 'png' });
  fs.writeFileSync('G:/test1.x/_perf/code_fullscreen.png', Buffer.from(shot.data, 'base64'));
  console.log('screenshot saved');

  // Esc 关闭验证
  await cdp(ws, 'Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
  await cdp(ws, 'Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
  await sleep(800);
  const closed = await evalJs(`JSON.stringify({ maskGone: !document.querySelector('.code-fs-mask') })`);
  console.log('AFTER ESC', closed);
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
