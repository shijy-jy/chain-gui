// CDP 端到端验证：选工作区 → 文件树模式 → 点图谱节点 → 侧栏只读断言
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
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.text + ' ' + (r.exceptionDetails.exception?.description || ''));
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

  // 1) 选「engram」工作区（点击名称为 engram 的行）
  const clicked = await evalJs(ws, `(() => {
    const span = [...document.querySelectorAll('*')].find(e => e.children.length === 0 && e.textContent.trim() === 'engram');
    if (!span) return { ok: false, why: 'no row' };
    let el = span;
    for (let i = 0; i < 4; i++) { if (el.parentElement) el = el.parentElement; else break; }
    (el.click ? el : span).click();
    return { ok: true, tag: el.tagName, cls: el.className };
  })()`);
  console.log('select ws:', JSON.stringify(clicked));
  await sleep(2500);

  const afterSel = await evalJs(ws, `(() => ({
    nodes: window.__engramDebug?.snapshot ? (typeof window.__engramDebug.snapshot === 'function' ? window.__engramDebug.snapshot().nodes.length : 'obj') : 'none',
    cyNodes: window.__engramDebug?.cy ? window.__engramDebug.cy.nodes().length : 'none',
    text: document.body.innerText.includes('engram-framework') || document.body.innerText.slice(0, 400),
  }))()`);
  console.log('after select:', JSON.stringify(afterSel).slice(0, 400));

  // 2) 打开文件树模式
  const rm = await evalJs(ws, `(() => {
    const b = [...document.querySelectorAll('button')].find(x => x.textContent.includes('文件树'));
    if (!b) return { ok: false };
    b.click(); return { ok: true };
  })()`);
  console.log('open read-mode:', JSON.stringify(rm));
  await sleep(1500);

  const rmState = await evalJs(ws, `(() => {
    const txt = document.body.innerText;
    return {
      hasTree: txt.includes('节点文件树'),
      hasNew: /\+?\s*新建/.test(txt),
      hasEdit: /✎|编辑/.test(txt),
      hasDel: /🗑|删除/.test(txt),
      hasPrevNext: txt.includes('下一篇'),
      hasRenderToggle: txt.includes('原文'),
    };
  })()`);
  console.log('read-mode state:', JSON.stringify(rmState, null, 1));

  // 3) 回到图谱，点击画布中心选一个节点，探侧栏
  await evalJs(ws, `(() => {
    const b = [...document.querySelectorAll('button')].find(x => x.textContent.includes('图谱'));
    if (b) b.click();
  })()`);
  await sleep(1200);
  const rect = await evalJs(ws, `(() => {
    const c = document.querySelector('.cy-container');
    if (!c) return null;
    const r = c.getBoundingClientRect();
    return { x: r.left + r.width / 2, y: r.top + r.height / 2, w: r.width, h: r.height };
  })()`);
  console.log('canvas rect:', JSON.stringify(rect));
  if (rect) {
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: rect.x, y: rect.y, button: 'left', clickCount: 1 });
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: rect.x, y: rect.y, button: 'left', clickCount: 1 });
    await sleep(1200);
  }
  const sb = await evalJs(ws, `(() => {
    const txt = document.body.innerText;
    return {
      hasSidebarTitle: txt.includes('检索线索') || txt.includes('代码'),
      hasEditForm: /保存|Ctrl\+S/.test(txt),
      hasFoldBtn: txt.includes('折叠子链'),
      hasDelBtn: txt.includes('删除节点'),
      hasMemory: txt.includes('强度') || txt.includes('触发'),
      sample: txt.slice(-600),
    };
  })()`);
  console.log('sidebar state:', JSON.stringify(sb, null, 1));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
