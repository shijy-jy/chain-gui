// 2.14.0 验证：点击 ⧉ → 新窗口出现 → 截图
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
async function connect(url) {
  const ws = new WebSocket(url);
  const pend = new Map();
  let sid = 0;
  ws.onmessage = (msg) => {
    const data = JSON.parse(msg.data);
    if (data.id && pend.has(data.id)) {
      const p = pend.get(data.id);
      pend.delete(data.id);
      data.error ? p.reject(new Error(data.error.message)) : p.resolve(data.result);
    }
  };
  await new Promise((res) => { ws.onopen = res; });
  const call = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = ++sid;
      pend.set(id, { resolve, reject });
      ws.send(JSON.stringify({ id, method, params }));
    });
  await call('Runtime.enable');
  const evalJs = async (expression, awaitPromise = false) => {
    const r = await call('Runtime.evaluate', { expression, awaitPromise, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.text + ' ' + (r.exceptionDetails.exception?.description ?? ''));
    return r.result.value;
  };
  return { ws, evalJs };
}

(async () => {
  const targets = await getJson('http://127.0.0.1:9222/json/list');
  const mainPage = targets.find((t) => t.type === 'page' && !t.url.includes('view=code'));
  const main = await connect(mainPage.webSocketDebuggerUrl);

  // 打开 engram + 点击 engram-core-map
  await main.evalJs(`(() => {
    const tab = Array.from(document.querySelectorAll('.mode-tab')).find((t) => t.textContent.includes('开发'));
    if (tab && !tab.className.includes('active')) { tab.click(); }
    return 'tab';
  })()`);
  await sleep(800);
  await main.evalJs(`(() => {
    const row = Array.from(document.querySelectorAll('.ws-row')).find((r) => r.textContent.includes('G:\\\\engram'));
    if (row) { row.click(); return 'clicked'; }
    return 'NO ROW';
  })()`);
  await sleep(6000);
  const pos = JSON.parse(await main.evalJs(`(() => {
    const d = window.__engramDebug;
    const p = d.cy.getElementById('engram-core-map').renderedPosition();
    const wrap = document.querySelector('.canvas-wrap');
    const r = wrap.getBoundingClientRect();
    return JSON.stringify({ x: r.left + p.x, y: r.top + p.y });
  })()`));
  const m = main.ws;
  await m.send(JSON.stringify({ id: 999, method: 'Input.dispatchMouseEvent', params: { type: 'mousePressed', x: pos.x, y: pos.y, button: 'left', clickCount: 1 } }));
  await m.send(JSON.stringify({ id: 998, method: 'Input.dispatchMouseEvent', params: { type: 'mouseReleased', x: pos.x, y: pos.y, button: 'left', clickCount: 1 } }));
  await sleep(2500);

  // 点击 ⧉ 按钮
  const clickedBtn = await main.evalJs(`(() => {
    const btns = Array.from(document.querySelectorAll('.code-max-btn'));
    const btn = btns.find((b) => b.textContent.includes('⧉'));
    if (btn) { btn.click(); return 'clicked'; }
    return 'NO BTN';
  })()`);
  console.log('⧉ click:', clickedBtn);
  await sleep(5000);

  // 新窗口目标
  const targets2 = await getJson('http://127.0.0.1:9222/json/list');
  console.log('targets:', targets2.map((t) => t.title + ' | ' + t.url.slice(0, 90)).join('\n'));
  const codeWin = targets2.find((t) => t.url.includes('view=code'));
  if (!codeWin) {
    console.log('NO CODE WINDOW');
    process.exit(1);
  }
  const cv = await connect(codeWin.webSocketDebuggerUrl);
  const state = await cv.evalJs(`(() => {
    const t = document.body.innerText;
    return JSON.stringify({
      hasSkeletonTitle: t.includes('代码骨架'),
      hasNode: t.includes('engram-core-map'),
      hasMermaid: !!document.querySelector('.cv-mermaid'),
      hasBody: !!document.querySelector('.cv-body'),
      bodyLen: t.length,
      scrollable: document.documentElement.scrollHeight > window.innerHeight,
    });
  })()`);
  console.log('CODEWIN STATE', state);
  const shot = await cv.ws.send(JSON.stringify({ id: 997, method: 'Page.captureScreenshot', params: { format: 'png' } }));
  await sleep(1500);
  const resp = await new Promise((res) => {
    cv.ws.onmessage = (msg) => {
      const d = JSON.parse(msg.data);
      if (d.id === 997) res(d);
    };
    setTimeout(() => res(null), 4000);
  });
  if (resp && resp.result) {
    fs.writeFileSync('G:/test1.x/_perf/code_window.png', Buffer.from(resp.result.data, 'base64'));
    console.log('screenshot saved');
  } else {
    console.log('screenshot failed');
  }
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
