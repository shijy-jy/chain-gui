// CDP 求值（从文件读 JS，避免 PowerShell 引号地狱）
// 用法：node _cdp_eval_file.cjs <script.js> [--json]
//   脚本最后一行表达式的结果会被返回；支持 await（包在 async IIFE 里）
const http = require('http');
const fs = require('fs');
const PORT = process.env.CDP_PORT || '9223';

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
(async () => {
  const file = process.argv[2];
  if (!file) { console.error('用法: node _cdp_eval_file.cjs <script.js>'); process.exit(2); }
  const body = fs.readFileSync(file, 'utf8');
  const targets = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  const page = targets.find((t) => t.type === 'page');
  if (!page) throw new Error('no page target');
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
  // 包成 async IIFE，允许脚本里直接 await
  const expr = `(async () => {\n${body}\n})()`;
  const r = await cdp(ws, 'Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) {
    console.error('EXC:', r.exceptionDetails.text, r.exceptionDetails.exception?.description || '');
    process.exit(1);
  }
  const v = r.result.value;
  console.log(typeof v === 'string' ? v : JSON.stringify(v, null, 2));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
