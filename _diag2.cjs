// 定点实验：新建表单里 正文/标签 的绑定为什么没进后端（逐个字段单独测）
const http = require('http');
const fs = require('fs');
const path = require('path');
const PORT = process.env.CDP_PORT || '9225';
const WS = 'G:\\test1.x\\_scratch\\ws_dev';

function getJson(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => { let d = ''; res.on('data', (c) => (d += c)); res.on('end', () => resolve(JSON.parse(d))); }).on('error', reject);
  });
}
let seq = 0;
const pending = new Map();
function cdp(ws, method, params = {}) {
  return new Promise((resolve, reject) => { const id = ++seq; pending.set(id, { resolve, reject }); ws.send(JSON.stringify({ id, method, params })); });
}
(async () => {
  const targets = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  const page = targets.find((t) => t.type === 'page');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && pending.has(d.id)) { const p = pending.get(d.id); pending.delete(d.id); d.error ? p.reject(new Error(d.error.message)) : p.resolve(d.result); }
  };
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws')); });
  await cdp(ws, 'Runtime.enable');
  const ev = async (expression) => {
    const r = await cdp(ws, 'Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error('eval: ' + r.exceptionDetails.text + ' ' + (r.exceptionDetails.exception?.description || ''));
    return r.result.value;
  };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const typeInto = async (sel, text) => { await ev(`(document.querySelector(${JSON.stringify(sel)}).focus(), true)`); await cdp(ws, 'Input.insertText', { text }); };

  const out = {};
  for (let i = 0; i < 100; i++) { if (await ev(`!!document.querySelector('.ws-row')`)) break; await sleep(300); }
  await ev(`(async () => { for (let i = 0; i < 5 && document.querySelector('.rm-mask'); i++) { window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await new Promise(r => setTimeout(r, 250)); } return true; })()`);

  out.addWs = await ev(`(async () => {
    try { await window.__TAURI_INTERNALS__.invoke('add_workspace', { dir: ${JSON.stringify(WS)}, mode: 'dev' }); } catch (e) { /* 已存在 */ }
    return true;
  })()`);
  out.openWs = await ev(`(async () => {
    const row = [...document.querySelectorAll('.ws-row')].find(r => (r.querySelector('.ws-name')?.textContent || '').trim() === 'ws_dev');
    if (!row) return { ok: false, names: [...document.querySelectorAll('.ws-name')].map(x => x.textContent) };
    row.click();
    for (let i = 0; i < 250; i++) {
      const d = (document.querySelector('.dir')?.textContent || '').trim();
      if (d.toLowerCase().includes('ws_dev') && window.__engramDebug?.snapshot) break;
      await new Promise(r => setTimeout(r, 120));
    }
    await new Promise(r => setTimeout(r, 500));
    return { ok: true, dir: (document.querySelector('.dir')?.textContent || '').trim(), nodes: window.__engramDebug?.snapshot?.nodes?.length, mode: window.__engramDebug?.mode };
  })()`);
  out.enterTree = await ev(`(async () => {
    const b = document.querySelector('.read-toggle');
    if (b && b.textContent.includes('文件树')) b.click();
    for (let i = 0; i < 300; i++) { if (document.querySelector('.rm-mask')) break; await new Promise(r => setTimeout(r, 10)); }
    return !!document.querySelector('.rm-mask');
  })()`);

  // 实验 1：只填正文（第一个字段）
  out.caseBodyFirst = await (async () => {
    await ev(`(async () => { document.querySelector('.rt-tool.primary').click(); await new Promise(r => setTimeout(r, 300)); return true; })()`);
    await typeInto('#n-title', 'BODY_ONLY');
    await typeInto('#n-body', 'ONLY-BODY-TEXT');
    const domVals = await ev(`({ title: document.getElementById('n-title').value, body: document.getElementById('n-body').value })`);
    const created = await ev(`(async () => {
      const before = new Set(window.__engramDebug?.snapshot?.nodes?.map(n => n.id) || []);
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
      for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
      await new Promise(r => setTimeout(r, 500));
      const s = window.__engramDebug?.snapshot;
      const n = (s?.nodes || []).find(x => !before.has(x.id));
      return { id: n?.id, body: n?.body, tags: n?.tags, rev: n?.revision };
    })()`);
    const file = path.join(WS, '.chain', 'nodes', `${created?.id}.md`);
    created.fileBody = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '(missing)';
    return { domVals, created };
  })();

  // 实验 2：只填标签（第二个字段，前面已有一个字段）
  out.caseTagsSecond = await (async () => {
    await ev(`(async () => { document.querySelector('.rt-tool.primary').click(); await new Promise(r => setTimeout(r, 300)); return true; })()`);
    await typeInto('#n-title', 'TAGS_ONLY');
    await typeInto('#n-tags', 'TAGX,TAGY');
    const domVals = await ev(`({ title: document.getElementById('n-title').value, tags: document.getElementById('n-tags').value })`);
    const created = await ev(`(async () => {
      const before = new Set(window.__engramDebug?.snapshot?.nodes?.map(n => n.id) || []);
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
      for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
      await new Promise(r => setTimeout(r, 500));
      const s = window.__engramDebug?.snapshot;
      const n = (s?.nodes || []).find(x => !before.has(x.id));
      return { id: n?.id, body: n?.body, tags: n?.tags, rev: n?.revision };
    })()`);
    const file = path.join(WS, '.chain', 'nodes', `${created?.id}.md`);
    created.fileBody = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '(missing)';
    return { domVals, created };
  })();

  // 实验 3：标签用 JS 直接赋值 + 手动派发 input（非可信事件）
  out.caseTagsSynthetic = await (async () => {
    await ev(`(async () => { document.querySelector('.rt-tool.primary').click(); await new Promise(r => setTimeout(r, 300)); return true; })()`);
    await typeInto('#n-title', 'SYNTH_TAGS');
    await ev(`(() => { const el = document.getElementById('n-tags'); el.value = 'SYN1,SYN2'; el.dispatchEvent(new Event('input', { bubbles: true })); return el.value; })()`);
    const created = await ev(`(async () => {
      const before = new Set(window.__engramDebug?.snapshot?.nodes?.map(n => n.id) || []);
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
      for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
      await new Promise(r => setTimeout(r, 500));
      const s = window.__engramDebug?.snapshot;
      const n = (s?.nodes || []).find(x => !before.has(x.id));
      return { id: n?.id, body: n?.body, tags: n?.tags, rev: n?.revision };
    })()`);
    return { created };
  })();

  // 清理三个实验节点 + 移除工作区
  out.cleanup = await ev(`(async () => {
    const ids = window.__engramDebug?.snapshot?.nodes?.filter(n => ['BODY_ONLY','TAGS_ONLY','SYNTH_TAGS'].includes(n.title)).map(n => n.id) || [];
    for (const id of ids) {
      const row = document.getElementById('rt-' + id);
      row?.click(); await new Promise(r => setTimeout(r, 250));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
      await new Promise(r => setTimeout(r, 200));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
      await new Promise(r => setTimeout(r, 450));
    }
    try { await window.__TAURI_INTERNALS__.invoke('remove_workspace', { dir: ${JSON.stringify(WS)} }); } catch (e) {}
    return { nodes: window.__engramDebug?.snapshot?.nodes?.length ?? -1 };
  })()`);

  console.log(JSON.stringify(out, null, 2));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL:', e.message); process.exit(1); });
