// CDP 验证第 6 段：展开信息栏（点击右下「«」切换）并 dump 右侧面板全文
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

  // 点击所有「«」按钮里位置最靠右的那个（信息栏展开/收起）
  const clicked = await evalJs(ws, `(() => {
    const bs = [...document.querySelectorAll('button')].filter(b => b.textContent.trim() === '«');
    if (!bs.length) return { ok: false, why: 'no toggle' };
    const r = bs[bs.length - 1].getBoundingClientRect();
    bs[bs.length - 1].click();
    return { ok: true, at: { x: Math.round(r.left), y: Math.round(r.top) } };
  })()`);
  console.log('toggle:', JSON.stringify(clicked));
  await sleep(1200);

  const dump = await evalJs(ws, `(() => {
    const txt = document.body.innerText;
    const idx = txt.lastIndexOf('FFT海洋渲染');
    const around = idx >= 0 ? txt.slice(idx - 100, idx + 1200) : txt.slice(-1400);
    const hasSave = txt.includes('保存') || txt.includes('Ctrl');
    return {
      hasEditForm: hasSave,
      hasMemory: txt.includes('检索线索'),
      hasCodeTab: txt.includes('代码骨架') || txt.includes('代码'),
      hasTrigger: txt.includes('触发'),
      around: around,
    };
  })()`);
  console.log('sidebar:', JSON.stringify(dump, null, 1));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
