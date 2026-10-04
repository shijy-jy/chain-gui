// 定点诊断：新建表单里的 正文/标签 是否真的写进节点文件（用 CDP 真实输入 insertText）
const http = require('http');
const fs = require('fs');
const path = require('path');
const PORT = process.env.CDP_PORT || '9225';
const WS_DEV = 'G:\\test1.x\\_scratch\\ws_dev';

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
const out = {};
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
  const typeInto = async (selector, text) => {
    await ev(`(document.querySelector(${JSON.stringify(selector)}).focus(), true)`);
    await cdp(ws, 'Input.insertText', { text });
  };

  for (let i = 0; i < 100; i++) { if (await ev(`!!document.querySelector('.ws-row')`)) break; await sleep(300); }
  await ev(`(async () => { for (let i = 0; i < 5 && document.querySelector('.rm-mask'); i++) { window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await new Promise(r => setTimeout(r, 250)); } return true; })()`);

  out.addWs = await ev(`(async () => {
    try { const list = await window.__TAURI_INTERNALS__.invoke('add_workspace', { dir: ${JSON.stringify(WS_DEV)}, mode: 'dev' }); return { ok: true, n: list.length }; }
    catch (e) { return { ok: false, err: String(e) }; }
  })()`);
  out.openWs = await ev(`(async () => {
    const row = [...document.querySelectorAll('.ws-row')].find(r => (r.getAttribute('title') || '') === ${JSON.stringify(WS_DEV)});
    if (!row) return { ok: false };
    row.click();
    for (let i = 0; i < 200; i++) { if ((document.querySelector('.dir')?.textContent || '').trim() === ${JSON.stringify(WS_DEV)}) break; await new Promise(r => setTimeout(r, 120)); }
    await new Promise(r => setTimeout(r, 500));
    return { ok: true, nodes: window.__engramDebug?.snapshot?.nodes?.length ?? -1 };
  })()`);
  out.enterTree = await ev(`(async () => {
    const b = document.querySelector('.read-toggle');
    if (b && b.textContent.includes('文件树')) b.click();
    for (let i = 0; i < 300; i++) { if (document.querySelector('.rm-mask')) break; await new Promise(r => setTimeout(r, 10)); }
    document.querySelector('.rt-tool.primary').click();
    await new Promise(r => setTimeout(r, 300));
    return { overlay: !!document.querySelector('.rm-mask'), form: !!document.querySelector('.rd-form') };
  })()`);

  // 真实输入（trusted events）
  await typeInto('#n-title', '真打字新建');
  await typeInto('#n-id', 'diag-1');
  await typeInto('#n-tags', '打字标签A, 打字标签B');
  await typeInto('#n-body', '# 真打字新建\n\n这是用 CDP Input.insertText 真实输入的正文。');
  out.formValues = await ev(`({
    title: document.getElementById('n-title').value,
    id: document.getElementById('n-id').value,
    tags: document.getElementById('n-tags').value,
    body: document.getElementById('n-body').value,
  })`);

  out.create = await ev(`(async () => {
    const before = new Set(window.__engramDebug?.snapshot?.nodes?.map(n => n.id) || []);
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
    for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 600));
    const s = window.__engramDebug?.snapshot;
    const fresh = s?.nodes?.filter(n => !before.has(n.id)) || [];
    const n = fresh[0];
    return {
      freshIds: fresh.map(x => x.id),
      title: n?.title, body: n?.body, tags: n?.tags, rev: n?.revision, parent: n?.parent,
      formError: document.querySelector('.rd-formerr')?.textContent?.trim() || null,
      formMsg: document.querySelector('.rd-msg')?.textContent?.trim() || null,
      stillForm: !!document.querySelector('.rd-form'),
    };
  })()`);

  // 磁盘真相
  const file = path.join(WS_DEV, '.chain', 'nodes', 'diag-1.md');
  out.fileOnDisk = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '(missing)';

  // 清理：删掉诊断节点 + 移除工作区
  out.cleanup = await ev(`(async () => {
    try {
      const row = document.getElementById('rt-diag-1');
      row?.click();
      await new Promise(r => setTimeout(r, 300));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
      await new Promise(r => setTimeout(r, 200));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
      await new Promise(r => setTimeout(r, 600));
      await window.__TAURI_INTERNALS__.invoke('remove_workspace', { dir: ${JSON.stringify(WS_DEV)} });
      return { nodes: window.__engramDebug?.snapshot?.nodes?.length ?? -1, removed: true };
    } catch (e) { return { err: String(e) }; }
  })()`);

  console.log(JSON.stringify(out, null, 2));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL:', e.message); console.log(JSON.stringify(out, null, 2)); process.exit(1); });
