// 2.16.0 验证：RESTRI 分析模式 → task 节点 openLoop 标记数
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
  // 打开 RESTRI（分析页签）
  await evalJs(`(() => {
    const tab = Array.from(document.querySelectorAll('.mode-tab')).find((t) => t.textContent.includes('分析'));
    if (tab && !tab.className.includes('active')) { tab.click(); }
    return 'tab';
  })()`);
  await sleep(800);
  await evalJs(`(() => {
    const row = Array.from(document.querySelectorAll('.ws-row')).find((r) => r.textContent.includes('RESTRI'));
    if (row) { row.click(); return 'clicked'; }
    return 'NO ROW';
  })()`);
  await sleep(6000);
  const info = await evalJs(`(() => {
    const d = window.__engramDebug;
    const cy = d.cy;
    let open = 0, tasks = 0;
    cy.nodes().forEach((n) => {
      if (n.data('nodeType') === 'task') { tasks++; if (n.data('openLoop')) open++; }
    });
    const t002 = cy.getElementById('t-002');
    return JSON.stringify({ tasks, openLoop: open, t002OpenLoop: t002.data('openLoop') });
  })()`);
  console.log('OPENLOOP', info);
  const shot = await cdp(ws, 'Page.captureScreenshot', { format: 'png' });
  require('fs').writeFileSync('G:/test1.x/_perf/openloop.png', Buffer.from(shot.data, 'base64'));
  console.log('screenshot saved');
  process.exit(0);
})().catch((e) => { console.error('FAIL', e); process.exit(1); });
