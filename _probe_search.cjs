// 探针：编辑面「父节点搜索」下拉为何不出候选（合成事件 vs 真实点击+输入）
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
  await cdp(ws, 'Page.enable');
  const ev = async (expression) => {
    const r = await cdp(ws, 'Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error('eval: ' + r.exceptionDetails.text + ' ' + (r.exceptionDetails.exception?.description || ''));
    return r.result.value;
  };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const rect = (sel) => ev(`(() => { const el = document.querySelector(${JSON.stringify(sel)}); if (!el) return null; const r = el.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
  const clickAt = async (p) => {
    if (!p) return false;
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x: p.x, y: p.y, button: 'left', clickCount: 1 });
    await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x: p.x, y: p.y, button: 'left', clickCount: 1 });
    return true;
  };

  for (let i = 0; i < 120; i++) { if (await ev(`!!document.querySelector('.ws-row') && !!window.__engramDebug?.snapshot`)) break; await sleep(250); }
  await ev(`(async () => { for (let i = 0; i < 5 && document.querySelector('.rm-mask'); i++) { window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await new Promise(r => setTimeout(r, 300)); } return true; })()`);
  await ev(`(async () => { try { await window.__TAURI_INTERNALS__.invoke('add_workspace', { dir: ${JSON.stringify(WS)}, mode: 'dev' }); } catch (e) {} return true; })()`);
  await ev(`(() => { localStorage.setItem('chain-gui-last-dir', ${JSON.stringify(WS)}); localStorage.setItem('chain-gui-mode', 'dev'); return true; })()`);
  await cdp(ws, 'Page.reload', {});
  await sleep(2000);
  for (let i = 0; i < 120; i++) { if (await ev(`!!window.__engramDebug?.snapshot`)) break; await sleep(250); }
  out.state = await ev(`(() => ({ dir: (document.querySelector('.dir')?.textContent || '').trim(), nodes: window.__engramDebug?.snapshot?.nodes?.length }))()`);

  // 进文件树模式 → 新建两个节点（父 P、子 C）
  await ev(`(async () => {
    const b = document.querySelector('.read-toggle'); if (b && b.textContent.includes('文件树')) b.click();
    for (let i = 0; i < 300; i++) { if (document.querySelector('.rm-mask')) break; await new Promise(r => setTimeout(r, 10)); }
    return true;
  })()`);
  const mkNode = async (id, title) => ev(`(async () => {
    document.querySelector('.rt-tool.primary').click();
    await new Promise(r => setTimeout(r, 250));
    const setV = (i, v) => { const el = document.getElementById(i); el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); };
    setV('n-title', ${JSON.stringify(title)});
    setV('n-id', ${JSON.stringify(id)});
    setV('n-body', 'body of ${id}');
    const indep = [...document.querySelectorAll('.parent-clear')].find(b => b.textContent.includes('独立节点'));
    indep?.click();
    await new Promise(r => setTimeout(r, 150));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
    for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 900));
    return !!window.__engramDebug?.snapshot?.nodes?.some(n => n.id === ${JSON.stringify(id)});
  })()`);
  out.p1 = await mkNode('probe-p', '探针父');
  out.p2 = await mkNode('probe-c', '探针子');

  // 选中 probe-c → 编辑 → 用「合成事件」搜父
  out.editSynthetic = await ev(`(async () => {
    document.getElementById('rt-probe-c')?.click();
    await new Promise(r => setTimeout(r, 350));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('编辑')).click();
    await new Promise(r => setTimeout(r, 350));
    const s = document.querySelector('.rd-form .parent-search');
    if (!s) return { why: 'no search input' };
    s.focus();
    await new Promise(r => setTimeout(r, 120));
    const afterFocus = { active: document.activeElement === s, results: document.querySelectorAll('.parent-opt').length, resultsBox: !!document.querySelector('.parent-results') };
    s.value = 'probe-p';
    s.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise(r => setTimeout(r, 300));
    return { afterFocus, options: [...document.querySelectorAll('.parent-opt')].map(o => o.textContent.trim()), box: !!document.querySelector('.parent-results'), query: s.value };
  })()`);

  // 用「真实点击 + insertText」再试一次
  out.editReal = await (async () => {
    const p = await rect('.rd-form .parent-search');
    await clickAt(p);
    await sleep(200);
    const activeType = await ev(`(() => ({ active: document.activeElement?.className || null, box: !!document.querySelector('.parent-results') }))()`);
    await cdp(ws, 'Input.insertText', { text: 'probe' });
    await sleep(400);
    const after = await ev(`(() => ({ query: document.querySelector('.rd-form .parent-search')?.value, options: [...document.querySelectorAll('.parent-opt')].map(o => o.textContent.trim()), box: !!document.querySelector('.parent-results') }))()`);
    return { activeType, after };
  })();

  out.cleanup = await ev(`(async () => {
    [...document.querySelectorAll('.rd-btn')].find(x => x.textContent.includes('取消'))?.click();
    await new Promise(r => setTimeout(r, 300));
    for (const id of ['probe-c', 'probe-p']) {
      const row = document.getElementById('rt-' + id);
      row?.click(); await new Promise(r => setTimeout(r, 300));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
      await new Promise(r => setTimeout(r, 200));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
      await new Promise(r => setTimeout(r, 800));
    }
    const s = window.__engramDebug?.snapshot;
    return { nodes: s?.nodes?.length, leftover: s?.nodes?.filter(n => n.id.startsWith('probe-')).map(n => n.id) };
  })()`);

  console.log(JSON.stringify(out, null, 2));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL:', e.message); console.log(JSON.stringify(out, null, 2)); process.exit(1); });
