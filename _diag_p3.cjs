// CDP 验证：对话阅读面（P3）——点 💬 按钮，断言只读阅读面 DOM
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
  // 等 CDP 就绪（app 可能尚未启动，重试 15 次）
  let targets = null;
  for (let i = 0; i < 15; i++) {
    try { targets = await getJson(`http://127.0.0.1:${PORT}/json/list`); break; }
    catch (e) { await sleep(1000); }
  }
  if (!targets) { console.log('CDP 不可用'); process.exit(2); }
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
  await cdp(ws, 'Page.enable');
  await cdp(ws, 'Page.reload', { ignoreCache: true });
  await sleep(7000);

  const base = await evalJs(ws, `(() => ({
    readyState: document.readyState,
    hasDialogueBtn: [...document.querySelectorAll('button')].some(b => b.textContent.includes('💬') || b.title.includes('对话')),
    bodyLen: document.body.innerHTML.length,
  }))()`);
  console.log('base:', JSON.stringify(base));

  // 点 💬 按钮
  const clicked = await evalJs(ws, `(() => {
    const b = [...document.querySelectorAll('button')].find(x => x.textContent.includes('💬') || x.title.includes('对话'));
    if (!b) return { ok: false };
    b.click(); return { ok: true };
  })()`);
  console.log('click:', JSON.stringify(clicked));
  await sleep(1500);

  const dlg = await evalJs(ws, `(() => {
    const txt = document.body.innerText;
    return {
      hasTitle: txt.includes('对话 · 工作过程'),
      hasSessions: txt.includes('会话'),
      hasSearch: txt.includes('检索对话'),
      hasWrite: txt.includes('保存') || txt.includes('新建') || txt.includes('删除'),
      hasNoLedgerHint: txt.includes('还没有对话账本') || txt.includes('条记录'),
      sample: txt.slice(-700),
    };
  })()`);
  console.log('dialogue-reader:', JSON.stringify(dlg, null, 1));

  // Esc 关闭
  await cdp(ws, 'Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
  await cdp(ws, 'Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
  await sleep(600);
  const afterEsc = await evalJs(ws, `document.body.innerText.includes('对话 · 工作过程')`);
  console.log('esc-closed:', !afterEsc);
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
