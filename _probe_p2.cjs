// 只读 CDP 探针：确认连接 + 点击「文件树模式」按钮（人专用视图的入口）后回报 DOM 状态
// 用法：node _probe_p2.cjs
const http = require('http');
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
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

(async () => {
  let targets;
  try {
    targets = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  } catch (e) {
    console.log(`CDP 不可用：${e.message}`);
    process.exit(2);
  }
  const page = targets.find((t) => t.type === 'page');
  if (!page) { console.log('没有 page 目标'); process.exit(2); }
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

  // 1) 工具栏现状：找「＋节点」新建按钮（应该已不存在）与「文件树模式」按钮
  const r1 = await cdp(ws, 'Runtime.evaluate', {
    expression: `(() => {
      const btns = [...document.querySelectorAll('button')].map(b => b.textContent.trim()).filter(t => t.length > 0 && t.length < 12);
      const toolbar = [...document.querySelectorAll('header button')].map(b => b.textContent.trim());
      return {
        toolbar: toolbar,
        hasCreateNode: toolbar.some(t => t.includes('节点')),
        hasReadMode: toolbar.some(t => t.includes('文件树')),
        debug: Object.keys(window.__engramDebug || {}),
      };
    })()`,
    returnByValue: true,
  });

  // 2) 点击「文件树模式」按钮（若存在）
  const r2 = await cdp(ws, 'Runtime.evaluate', {
    expression: `(() => {
      const btn = [...document.querySelectorAll('header button')].find(b => b.textContent.includes('文件树'));
      if (!btn) return { clicked: false, reason: 'no button' };
      btn.click();
      return { clicked: true };
    })()`,
    returnByValue: true,
  });
  await sleep(1200);

  // 3) 阅读面现状：找新建/编辑/删除按钮（应该都不存在）
  const r3 = await cdp(ws, 'Runtime.evaluate', {
    expression: `(() => {
      const txt = document.body.innerText;
      return {
        hasNew: /新建/.test(txt),
        hasEdit: /编辑/.test(txt),
        hasDeleteBtn: /删除/.test(txt),
        hasFileTree: txt.includes('节点文件树'),
        hasReadPane: txt.includes('上一篇') || txt.includes('下一篇') || txt.includes('原文'),
        totalTextLen: txt.length,
      };
    })()`,
    returnByValue: true,
  });

  console.log('toolbar:', JSON.stringify(r1.result.value, null, 1));
  console.log('click:', JSON.stringify(r2.result.value));
  console.log('reader-mode:', JSON.stringify(r3.result.value, null, 1));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
