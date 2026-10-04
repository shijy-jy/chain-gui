// CDP 验证第 4 段：信息栏面板级断言（选中节点后：正文预览/检索线索/代码维护按钮在，编辑表单不在）
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

  const sb = await evalJs(ws, `(() => {
    const btns = [...document.querySelectorAll('button')].map(b => b.textContent.trim()).filter(t => t.length > 0);
    const inputs = document.querySelectorAll('input, textarea, select').length;
    const title = [...document.querySelectorAll('h1,h2,h3,.sb-title,.node-title')].map(e => e.textContent.trim()).filter(Boolean).slice(0, 5);
    return {
      tabButtons: btns.filter(t => t.includes('检索线索') || t.includes('代码') || t.includes('正文') || t.includes('过程日志')),
      hasAnyInput: inputs,
      writeButtons: btns.filter(t => t.includes('保存') || t.includes('删除') || t.includes('折叠') || t.includes('挂载到') || t.includes('改挂')),
      maintainButtons: btns.filter(t => t.includes('挂载源码') || t.includes('刷新骨架') || t.includes('展开全屏') || t.includes('添加日志')),
      titles: title,
    };
  })()`);
  console.log('sidebar-panels:', JSON.stringify(sb, null, 1));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
