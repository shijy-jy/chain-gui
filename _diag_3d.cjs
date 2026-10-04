// CDP 验证：三维图谱视图——WebGL canvas、无 cy、无波纹、交互提示文案、悬停/选中冒烟
const http = require('http');
const PORT = '9223';
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
async function evalJs(ws, expr) {
  const r = await cdp(ws, 'Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  return r.result.value;
}

(async () => {
  let targets = null;
  for (let i = 0; i < 10; i++) {
    try { targets = await getJson(`http://127.0.0.1:${PORT}/json/list`); break; }
    catch (e) { await sleep(1000); }
  }
  if (!targets) { console.log('CDP 不可用（app 未启动）'); process.exit(2); }
  const page = targets.find((t) => t.type === 'page');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  const errors = [];
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && pending.has(d.id)) {
      const p = pending.get(d.id);
      pending.delete(d.id);
      d.error ? p.reject(new Error(d.error.message)) : p.resolve(d.result);
      return;
    }
    if (d.method === 'Runtime.exceptionThrown') {
      errors.push('EXCEPTION: ' + (d.params.exceptionDetails.exception?.description || d.params.exceptionDetails.text || '').slice(0, 300));
    }
  };
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws')); });
  await cdp(ws, 'Runtime.enable');
  await cdp(ws, 'Page.enable');
  await cdp(ws, 'Page.reload', { ignoreCache: true });
  await sleep(9000);

  const state = await evalJs(ws, `(() => {
    const txt = document.body.innerText;
    return {
      readyState: document.readyState,
      bodyLen: document.body.innerHTML.length,
      hasWebGLCanvas: !!document.querySelector('canvas'),
      debugKeys: window.__engramDebug ? Object.keys(window.__engramDebug) : null,
      hasCyContainer: !!document.querySelector('.cy-container'),
      hasWater: txt.includes('波纹参数') || txt.includes('水面波场') || txt.includes('亮度对比'),
      hasUnityHint: txt.includes('右键旋转') || txt.includes('旋转视角') || txt.includes('双击聚焦'),
      hasToolbar3D: txt.includes('复制 AI 指南'),
      hasReadModeBtn: txt.includes('文件树'),
    };
  })()`);
  console.log('state:', JSON.stringify(state, null, 1));

  // 点 canvas 中心：冒烟选中/悬停不炸（不断言结果，只看有无异常）
  const rect = await evalJs(ws, `(() => {
    const c = document.querySelector('canvas');
    if (!c) return null;
    const r = c.getBoundingClientRect();
    return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2), w: Math.round(r.width), h: Math.round(r.height) };
  })()`);
  console.log('canvas:', JSON.stringify(rect));
  if (rect && rect.w > 50) {
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseMoved', x: rect.x, y: rect.y });
    await sleep(300);
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: rect.x, y: rect.y, button: 'left', clickCount: 1 });
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: rect.x, y: rect.y, button: 'left', clickCount: 1 });
    await sleep(500);
    // 双击聚焦
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: rect.x, y: rect.y, button: 'left', clickCount: 2 });
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: rect.x, y: rect.y, button: 'left', clickCount: 2 });
    await sleep(500);
    // 滚轮缩放
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseWheel', x: rect.x, y: rect.y, deltaX: 0, deltaY: -120 });
    await sleep(500);
    // 右键旋转拖拽
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: rect.x, y: rect.y, button: 'right', clickCount: 1 });
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseMoved', x: rect.x + 120, y: rect.y + 40, buttons: 2 });
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: rect.x + 120, y: rect.y + 40, button: 'right', clickCount: 1 });
    await sleep(600);
  }
  console.log('interaction errors:', errors.length ? errors.join('\n') : '(none)');
  const after = await evalJs(ws, `(() => {
    const txt = document.body.innerText;
    const sel = [...document.querySelectorAll('h1,h2,h3,.sb-title,.node-title,.ro-value')].map(e=>e.textContent.trim()).filter(Boolean).slice(0,6);
    return { textTail: txt.slice(-500), titles: sel };
  })()`);
  console.log('after:', JSON.stringify(after, null, 1));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
