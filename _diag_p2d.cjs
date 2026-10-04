// CDP 验证第 2 段：文件树模式只读断言 + 侧栏只读断言（纯字符串判断，避免正则转义坑）
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
  await cdp(ws, 'Runtime.enable');

  // 文件树模式状态（当前应已打开）
  const rm = await evalJs(ws, `(() => {
    const txt = document.body.innerText;
    return {
      hasTree: txt.includes('节点文件树'),
      hasNew: txt.includes('新建'),
      hasEdit: txt.includes('编辑'),
      hasDel: txt.includes('删除'),
      hasPrevNext: txt.includes('下一篇'),
      hasRenderToggle: txt.includes('原文'),
      hasLocate: txt.includes('在图谱中定位'),
    };
  })()`);
  console.log('read-mode:', JSON.stringify(rm, null, 1));

  // 返回图谱
  await evalJs(ws, `(() => { const b = [...document.querySelectorAll('button')].find(x => x.textContent.includes('图谱')); if (b) b.click(); return !!b; })()`);
  await sleep(1200);

  // 点击画布中心选中一个节点（water 工作区 31 节点）
  const rect = await evalJs(ws, `(() => {
    const c = document.querySelector('.cy-container');
    if (!c) return null;
    const r = c.getBoundingClientRect();
    return { x: Math.round(r.left + r.width / 2), y: Math.round(r.top + r.height / 2) };
  })()`);
  if (rect) {
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: rect.x, y: rect.y, button: 'left', clickCount: 1 });
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: rect.x, y: rect.y, button: 'left', clickCount: 1 });
    await sleep(1500);
  }

  const sb = await evalJs(ws, `(() => {
    const txt = document.body.innerText;
    const hasSave = txt.includes('保存');
    const hasCtrlS = txt.includes('Ctrl');
    return {
      sidebarVisible: txt.includes('检索线索') || txt.includes('代码'),
      hasMemoryInfo: txt.includes('触发') || txt.includes('强度') || txt.includes('触达'),
      hasEditForm: hasSave || hasCtrlS || txt.includes('标题') && txt.includes('状态') && txt.includes('正文'),
      hasFold: txt.includes('折叠'),
      hasDelNode: txt.includes('删除节点'),
      selected: window.__engramDebug && window.__engramDebug.snapshot ? 'has-debug' : 'no-debug',
      sample: txt.slice(0, 700),
    };
  })()`);
  console.log('sidebar:', JSON.stringify(sb, null, 1));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
