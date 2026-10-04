// v2.20 终验：文件树模式新建/编辑/删除/改挂载（含改挂载与成环护栏）
// 修正：用 localStorage + 页面重载把临时工作区开起来（App 的工作区列表只在启动时拉取）
const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = process.env.CDP_PORT || '9225';
const SHOTS = path.join(__dirname, '_shots');
const WS_DEV = 'G:\\test1.x\\_scratch\\ws_dev';
const WS_ANA = 'G:\\test1.x\\_scratch\\ws_ana';

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
const R = {};
const exceptions = [];
(async () => {
  fs.mkdirSync(SHOTS, { recursive: true });
  const targets = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  const page = targets.find((t) => t.type === 'page');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  ws.onmessage = (msg) => {
    const d = JSON.parse(msg.data);
    if (d.id && pending.has(d.id)) { const p = pending.get(d.id); pending.delete(d.id); d.error ? p.reject(new Error(d.error.message)) : p.resolve(d.result); return; }
    if (d.method === 'Runtime.exceptionThrown') exceptions.push(d.params.exceptionDetails.exception?.description || d.params.exceptionDetails.text);
  };
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws error')); });
  await cdp(ws, 'Runtime.enable');
  await cdp(ws, 'Page.enable');

  const ev = async (expression) => {
    const r = await cdp(ws, 'Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error('eval: ' + r.exceptionDetails.text + ' ' + (r.exceptionDetails.exception?.description || ''));
    return r.result.value;
  };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const shot = async (name) => {
    const r = await cdp(ws, 'Page.captureScreenshot', { format: 'png' });
    fs.writeFileSync(path.join(SHOTS, name + '.png'), Buffer.from(r.data, 'base64'));
  };
  const readFile = (root, id) => {
    const p = path.join(root, '.chain', 'nodes', `${id}.md`);
    return fs.existsSync(p) ? fs.readFileSync(p, 'utf8') : '(missing)';
  };
  const waitReady = async () => {
    for (let i = 0; i < 120; i++) { if (await ev(`!!document.querySelector('.ws-row') && !!window.__engramDebug?.snapshot`)) return true; await sleep(250); }
    return false;
  };
  const reloadInto = async (dir, mode) => {
    await ev(`(() => { localStorage.setItem('chain-gui-last-dir', ${JSON.stringify(dir)}); localStorage.setItem('chain-gui-mode', ${JSON.stringify(mode)}); return true; })()`);
    await cdp(ws, 'Page.reload', { ignoreCache: false });
    await sleep(1500);
    await waitReady();
    return ev(`(() => ({ dir: (document.querySelector('.dir')?.textContent || '').trim(), mode: window.__engramDebug?.mode, nodes: window.__engramDebug?.snapshot?.nodes?.length ?? -1 }))()`);
  };
  const enterTree = () => ev(`(async () => {
    const b = document.querySelector('.read-toggle');
    if (b && b.textContent.includes('文件树')) b.click();
    for (let i = 0; i < 300; i++) { if (document.querySelector('.rm-mask')) break; await new Promise(r => setTimeout(r, 10)); }
    await new Promise(r => requestAnimationFrame(r));
    return { overlay: !!document.querySelector('.rm-mask'), badge: document.querySelector('.rm-badge')?.textContent?.trim() };
  })()`);
  const fillNew = (fields) => ev(`(async () => {
    document.querySelector('.rt-tool.primary').click();
    await new Promise(r => setTimeout(r, 300));
    const setV = (id, v) => { const el = document.getElementById(id); if (el) { el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); } };
    const f = ${JSON.stringify(fields)};
    if (f.type) { const t = document.getElementById('n-type'); if (t) { t.value = f.type; t.dispatchEvent(new Event('change', { bubbles: true })); await new Promise(r => setTimeout(r, 150)); } }
    if (f.status) { const s = document.getElementById('n-status'); if (s) { s.value = f.status; s.dispatchEvent(new Event('change', { bubbles: true })); } }
    if (f.title) setV('n-title', f.title);
    if (f.id) setV('n-id', f.id);
    if (f.tags) setV('n-tags', f.tags);
    if (f.body) setV('n-body', f.body);
    if (f.independent) [...document.querySelectorAll('.parent-clear')].find(b => b.textContent.includes('独立节点'))?.click();
    await new Promise(r => setTimeout(r, 200));
    return { id: document.getElementById('n-id')?.value, title: document.getElementById('n-title')?.value };
  })()`);
  const clickCreate = () => ev(`(async () => {
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
    for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 1200));
    return { msg: document.querySelector('.rd-msg')?.textContent?.trim() || null, err: document.querySelector('.rd-formerr')?.textContent?.trim() || null };
  })()`);
  /** 在编辑面里改父节点（必须 focus 才会展开候选列表） */
  const reparentInEdit = (query) => ev(`(async () => {
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('编辑')).click();
    await new Promise(r => setTimeout(r, 350));
    const search = document.querySelector('.rd-form .parent-search');
    search.focus();
    search.value = ${JSON.stringify(query)};
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise(r => setTimeout(r, 350));
    const opts = [...document.querySelectorAll('.parent-opt')].map(o => o.textContent.trim());
    document.querySelector('.parent-opt')?.click();
    await new Promise(r => setTimeout(r, 200));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('保存')).click();
    await new Promise(r => setTimeout(r, 1200));
    return { options: opts, err: document.querySelector('.rd-formerr')?.textContent?.trim() || null, stillEditing: !!document.querySelector('.rd-form') };
  })()`);
  const cancelForm = () => ev(`(async () => { [...document.querySelectorAll('.rd-btn')].find(x => x.textContent.includes('取消'))?.click(); await new Promise(r => setTimeout(r, 300)); return !document.querySelector('.rd-form'); })()`);

  await waitReady();
  R.register = await ev(`(async () => {
    const out = [];
    for (const [dir, mode] of [[${JSON.stringify(WS_DEV)}, 'dev'], [${JSON.stringify(WS_ANA)}, 'analysis']]) {
      try { await window.__TAURI_INTERNALS__.invoke('add_workspace', { dir, mode }); out.push({ dir, ok: true }); }
      catch (e) { out.push({ dir, ok: false, err: String(e) }); }
    }
    return out;
  })()`);

  // ── A. 开发模式工作区 ─────────────────────────────────────────────────
  R.devOpen = await reloadInto(WS_DEV, 'dev');
  R.devEnter = await enterTree();
  R.devFill = await fillNew({ title: '文件树新建实测', id: 'vfy-a', tags: '实测, 临时', body: '# 文件树新建实测\n\n第一版正文（新建时写入）。' });
  await shot('v21-1-new-form-dev');
  R.devCreate = await clickCreate();
  R.devNode = await ev(`(() => { const n = window.__engramDebug?.snapshot?.nodes?.find(x => x.id === 'vfy-a'); const cy = window.__engramDebug?.cy;
    return { title: n?.title, bodyLen: (n?.body || '').length, tags: Object.values(n?.tags || {}), parent: n?.parent, rev: n?.revision,
             graphHas: cy ? cy.getElementById('vfy-a').nonempty() : false, treeRow: !!document.getElementById('rt-vfy-a') }; })()`);
  R.devNodeFile = readFile(WS_DEV, 'vfy-a');
  await shot('v21-2-created-dev');

  R.devSave = await ev(`(async () => {
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('编辑')).click();
    await new Promise(r => setTimeout(r, 300));
    const setV = (id, v) => { const el = document.getElementById(id); el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); };
    setV('f-title', '文件树新建实测·改');
    setV('f-body', '# 改过的正文\\n\\n第二版内容。');
    setV('f-tags', '实测, 已编辑');
    await new Promise(r => setTimeout(r, 150));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('保存')).click();
    for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 1200));
    const n = window.__engramDebug?.snapshot?.nodes?.find(x => x.id === 'vfy-a');
    return { title: n?.title, bodyLen: (n?.body || '').length, tags: Object.values(n?.tags || {}), rev: n?.revision };
  })()`);
  R.devSaveFile = readFile(WS_DEV, 'vfy-a');

  // 改挂载：新建 vfy-parent → 把 vfy-a 挂过去
  R.devParent = await fillNew({ title: '新父节点', id: 'vfy-parent', body: '父节点正文', independent: true });
  R.devParentCreate = await clickCreate();
  R.devReparent = await reparentInEdit('vfy-parent');
  R.devAfterReparent = await ev(`(() => { const s = window.__engramDebug?.snapshot; const n = s?.nodes?.find(x => x.id === 'vfy-a');
    return { parent: n?.parent ?? null, edges: s?.edges?.length, rel: n?.rel }; })()`);
  await shot('v21-3-reparent-dev');

  R.devDelete = await ev(`(async () => {
    document.getElementById('rt-vfy-a')?.click();
    await new Promise(r => setTimeout(r, 350));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除')).click();
    await new Promise(r => setTimeout(r, 200));
    const armed = [...document.querySelectorAll('.rd-btn')].some(b => b.textContent.includes('确认删除'));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除')).click();
    await new Promise(r => setTimeout(r, 1200));
    const s = window.__engramDebug?.snapshot;
    return { twoStep: armed, gone: !s?.nodes?.some(n => n.id === 'vfy-a'), nodes: s?.nodes?.length };
  })()`);
  R.devCleanup = await ev(`(async () => {
    const ids = window.__engramDebug?.snapshot?.nodes?.filter(n => n.id.startsWith('vfy-')).map(n => n.id) || [];
    for (const id of ids) {
      document.getElementById('rt-' + id)?.click(); await new Promise(r => setTimeout(r, 300));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
      await new Promise(r => setTimeout(r, 200));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
      await new Promise(r => setTimeout(r, 800));
    }
    const s = window.__engramDebug?.snapshot;
    return { nodes: s?.nodes?.length, leftover: s?.nodes?.filter(n => n.id.startsWith('vfy-')).map(n => n.id) };
  })()`);

  // ── B. 分析模式工作区（护栏）─────────────────────────────────────────
  R.anaOpen = await reloadInto(WS_ANA, 'analysis');
  R.anaEnter = await enterTree();
  R.anaRoot = await ev(`(() => { const s = window.__engramDebug?.snapshot; return { rootGoal: s?.nodes?.find(n => n.parent === null)?.id ?? null, nodes: s?.nodes?.length }; })()`);
  R.anaFill = await ev(`(async () => {
    const rootGoal = window.__engramDebug?.snapshot?.nodes?.find(n => n.parent === null)?.id;
    document.getElementById('rt-' + rootGoal)?.click();
    await new Promise(r => setTimeout(r, 350));
    return { selected: document.querySelector('.rd-meta .chip.id')?.textContent?.trim() };
  })()`);
  R.anaFill2 = await fillNew({ type: 'design', status: 'in_progress', title: '分析模式新建实测', tags: '实测', body: '# 分析模式新建实测\n\n人用通道：新建 → 进链 + 图谱同父长出。' });
  await shot('v21-4-new-form-analysis');
  R.anaCreate = await clickCreate();
  R.anaNode = await ev(`(() => { const s = window.__engramDebug?.snapshot; const n = s?.nodes?.find(x => x.title === '分析模式新建实测'); const cy = window.__engramDebug?.cy;
    return { id: n?.id, parent: n?.parent, type: n?.type, status: n?.status, bodyLen: (n?.body || '').length, tags: Object.values(n?.tags || {}), rev: n?.revision,
             graphHas: n && cy ? cy.getElementById(n.id).nonempty() : false, treeRow: n ? !!document.getElementById('rt-' + n.id) : false, valid: s?.validation?.valid }; })()`);
  R.anaNodeFile = readFile(WS_ANA, R.anaNode?.id || 'x');
  await shot('v21-5-created-analysis');

  // ① 删根护栏
  R.anaGuardRoot = await ev(`(async () => {
    const s = window.__engramDebug?.snapshot;
    const rootGoal = s?.nodes?.find(n => n.parent === null)?.id;
    document.getElementById('rt-' + rootGoal)?.click();
    await new Promise(r => setTimeout(r, 350));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
    await new Promise(r => setTimeout(r, 200));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
    await new Promise(r => setTimeout(r, 900));
    return { err: document.querySelector('.rd-formerr')?.textContent?.trim() || null, rootAlive: !!window.__engramDebug?.snapshot?.nodes?.some(n => n.id === rootGoal) };
  })()`);

  // ② 子节点 + 删有子的父护栏
  R.anaChild = await fillNew({ title: '护栏子节点', tags: '实测', body: '子节点正文' });
  R.anaChildCreate = await clickCreate();
  R.anaGuardChildren = await ev(`(async () => {
    const s = window.__engramDebug?.snapshot;
    const parentId = s?.nodes?.find(n => n.title === '分析模式新建实测')?.id;
    document.getElementById('rt-' + parentId)?.click();
    await new Promise(r => setTimeout(r, 350));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
    await new Promise(r => setTimeout(r, 200));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
    await new Promise(r => setTimeout(r, 900));
    return { err: document.querySelector('.rd-formerr')?.textContent?.trim() || null, parentAlive: !!window.__engramDebug?.snapshot?.nodes?.some(n => n.id === parentId) };
  })()`);
  await shot('v21-6-guards-analysis');

  // ③ 成环护栏：把根 goal 挂到它的后代下
  R.anaCycle = await reparentInEdit(R.anaNode?.id || '');
  R.anaCycleState = await ev(`(() => { const s = window.__engramDebug?.snapshot; const rootGoal = s?.nodes?.find(n => n.parent === null)?.id;
    return { rootGoal, rootParent: s?.nodes?.find(n => n.id === rootGoal)?.parent ?? null, valid: s?.validation?.valid }; })()`);
  await shot('v21-7-cycle-guard');
  await cancelForm();

  // ④ 删叶子（允许）→ 清场
  R.anaCleanup = await ev(`(async () => {
    const s = window.__engramDebug?.snapshot;
    const child = s?.nodes?.find(n => n.title === '护栏子节点')?.id;
    const parent = s?.nodes?.find(n => n.title === '分析模式新建实测')?.id;
    const order = [child, parent].filter(Boolean);
    const steps = [];
    for (const id of order) {
      document.getElementById('rt-' + id)?.click();
      await new Promise(r => setTimeout(r, 350));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
      await new Promise(r => setTimeout(r, 200));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
      await new Promise(r => setTimeout(r, 900));
      steps.push({ id, gone: !window.__engramDebug?.snapshot?.nodes?.some(n => n.id === id), err: document.querySelector('.rd-formerr')?.textContent?.trim() || null });
    }
    const s2 = window.__engramDebug?.snapshot;
    return { steps, nodes: s2?.nodes?.length, leftover: s2?.nodes?.filter(n => n.title.includes('实测') || n.title.includes('护栏')).map(n => n.id), valid: s2?.validation?.valid };
  })()`);
  R.anaExit = await ev(`(async () => { window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await new Promise(r => setTimeout(r, 350)); return !document.querySelector('.rm-mask'); })()`);

  R.removeWs = await ev(`(async () => {
    const out = [];
    for (const dir of [${JSON.stringify(WS_DEV)}, ${JSON.stringify(WS_ANA)}]) {
      try { await window.__TAURI_INTERNALS__.invoke('remove_workspace', { dir }); out.push({ dir, ok: true }); } catch (e) { out.push({ dir, ok: false, err: String(e) }); }
    }
    return out;
  })()`);
  R.exceptions = exceptions;
  console.log(JSON.stringify(R, null, 2));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL:', e.message); console.log(JSON.stringify(R, null, 2)); process.exit(1); });
