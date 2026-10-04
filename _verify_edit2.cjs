// v2.20 文件树模式「新建 + 编辑 + 删除 + 改挂载」实测（真实应用内 CDP 驱动）
// 修正版：工作区按页签+名字打开；正文/标签以磁盘文件为真相；护栏逐条真测。
// 用法：node _verify_edit2.cjs   （应用需带 --remote-debugging-port=9225 启动）
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
const consoleErrors = [];

(async () => {
  fs.mkdirSync(SHOTS, { recursive: true });
  const targets = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  const page = targets.find((t) => t.type === 'page');
  if (!page) throw new Error('no page target');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  ws.onmessage = (msg) => {
    const d = JSON.parse(msg.data);
    if (d.id && pending.has(d.id)) { const p = pending.get(d.id); pending.delete(d.id); d.error ? p.reject(new Error(d.error.message)) : p.resolve(d.result); return; }
    if (d.method === 'Runtime.exceptionThrown') exceptions.push(d.params.exceptionDetails.exception?.description || d.params.exceptionDetails.text);
    if (d.method === 'Runtime.consoleAPICalled' && d.params.type === 'error') consoleErrors.push((d.params.args || []).map((a) => a.value ?? a.description).join(' '));
  };
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws error')); });
  await cdp(ws, 'Runtime.enable');
  await cdp(ws, 'Page.enable');

  const ev = async (expression) => {
    const r = await cdp(ws, 'Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error('eval: ' + r.exceptionDetails.text + ' ' + (r.exceptionDetails.exception?.description || ''));
    return r.result.value;
  };
  const shot = async (name) => {
    const r = await cdp(ws, 'Page.captureScreenshot', { format: 'png' });
    fs.writeFileSync(path.join(SHOTS, name + '.png'), Buffer.from(r.data, 'base64'));
  };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const nodeFile = (root, id) => path.join(root, '.chain', 'nodes', `${id}.md`);
  const readFile = (root, id) => (fs.existsSync(nodeFile(root, id)) ? fs.readFileSync(nodeFile(root, id), 'utf8') : '(missing)');

  for (let i = 0; i < 100; i++) { if (await ev(`!!document.querySelector('.ws-row')`)) break; await sleep(300); }
  await ev(`(async () => { for (let i = 0; i < 5 && document.querySelector('.rm-mask'); i++) { window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await new Promise(r => setTimeout(r, 300)); } return true; })()`);

  // 注册两个临时工作区（开发 / 分析）
  R.setup = await ev(`(async () => {
    const out = [];
    for (const [dir, mode] of [${JSON.stringify(WS_DEV)}, 'dev'].length ? [[${JSON.stringify(WS_DEV)}, 'dev'], [${JSON.stringify(WS_ANA)}, 'analysis']] : []) {
      try { await window.__TAURI_INTERNALS__.invoke('add_workspace', { dir, mode }); out.push({ dir, ok: true }); }
      catch (e) { out.push({ dir, ok: false, err: String(e) }); }
    }
    return out;
  })()`);

  // 打开工作区：先切页签，再按名字点行
  const openWs = (name, tab) => ev(`(async () => {
    const t = [...document.querySelectorAll('.mode-tab')].find(x => x.textContent.includes(${JSON.stringify(tab)}));
    t.click();
    await new Promise(r => setTimeout(r, 400));
    const row = [...document.querySelectorAll('.ws-row')].find(r => (r.querySelector('.ws-name')?.textContent || '').trim() === ${JSON.stringify(name)});
    if (!row) return { ok: false, names: [...document.querySelectorAll('.ws-name')].map(x => x.textContent.trim()) };
    row.click();
    for (let i = 0; i < 250; i++) {
      const d = (document.querySelector('.dir')?.textContent || '').toLowerCase();
      if (d.includes(${JSON.stringify(name)}) && window.__engramDebug?.snapshot) break;
      await new Promise(r => setTimeout(r, 120));
    }
    await new Promise(r => setTimeout(r, 500));
    const s = window.__engramDebug?.snapshot;
    return { ok: true, dir: (document.querySelector('.dir')?.textContent || '').trim(), nodes: s?.nodes?.length ?? -1, mode: window.__engramDebug?.mode, rootGoal: s?.nodes?.find(n => n.parent === null)?.id ?? null };
  })()`);

  const enterTree = () => ev(`(async () => {
    const b = document.querySelector('.read-toggle');
    if (b && b.textContent.includes('文件树')) b.click();
    for (let i = 0; i < 300; i++) { if (document.querySelector('.rm-mask')) break; await new Promise(r => setTimeout(r, 10)); }
    await new Promise(r => requestAnimationFrame(r));
    return { overlay: !!document.querySelector('.rm-mask'), badge: document.querySelector('.rm-badge')?.textContent?.trim() };
  })()`);
  const exitTree = () => ev(`(async () => { window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await new Promise(r => setTimeout(r, 350)); return !document.querySelector('.rm-mask'); })()`);

  // ── A. 开发模式 ────────────────────────────────────────────────────────
  R.devOpen = await openWs('ws_dev', '开发');
  R.devEnter = await enterTree();

  R.devNew = await ev(`(async () => {
    document.querySelector('.rt-tool.primary').click();
    await new Promise(r => setTimeout(r, 300));
    const setV = (id, v) => { const el = document.getElementById(id); el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); };
    setV('n-title', '开发模式新建实测');
    setV('n-id', 'vfy-dev-1');
    setV('n-tags', '实测, 临时');
    setV('n-body', '# 开发模式新建实测\\n\\n正文由 CDP 写入：验证「文件树新建 → 同一父节点下图谱同步长出节点」。');
    await new Promise(r => setTimeout(r, 200));
    return { title: document.getElementById('n-title').value, bodyLen: document.getElementById('n-body').value.length, tags: document.getElementById('n-tags').value, parent: document.querySelector('.rd-form .parent-row .parent-search')?.placeholder };
  })()`);
  await shot('v20-1-new-dev');

  R.devCreated = await ev(`(async () => {
    const cyBefore = window.__engramDebug?.cy?.nodes().length ?? -1;
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
    for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 1500));   // 跨过自写窗口，检验不会闪回
    const s = window.__engramDebug?.snapshot;
    const n = s?.nodes?.find(x => x.id === 'vfy-dev-1');
    const cy = window.__engramDebug?.cy;
    return {
      inSnapshot: !!n, title: n?.title, bodyChars: (n?.body || '').length, tags: Object.values(n?.tags || {}), type: n?.type, status: n?.status,
      graphBefore: cyBefore, graphAfter: cy ? cy.nodes().length : -1,
      graphHasNode: cy ? cy.getElementById('vfy-dev-1').nonempty() : false,
      treeHasRow: !!document.getElementById('rt-vfy-dev-1'),
      readerId: (document.querySelector('.rd-meta .chip.id')?.textContent || '').trim(),
      msg: document.querySelector('.rd-msg')?.textContent?.trim() || null,
    };
  })()`);
  R.devCreatedFile = readFile(WS_DEV, 'vfy-dev-1');
  await shot('v20-2-created-dev');

  R.devEdit = await ev(`(async () => {
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('编辑')).click();
    await new Promise(r => setTimeout(r, 300));
    const setV = (id, v) => { const el = document.getElementById(id); el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); };
    setV('f-title', '开发模式新建实测·改');
    setV('f-body', '# 改过的正文\\n\\n第二版内容（编辑面写回）。');
    setV('f-tags', '实测, 已编辑');
    await new Promise(r => setTimeout(r, 150));
    return { form: !!document.querySelector('.rd-form') };
  })()`);
  await shot('v20-3-edit-dev');

  R.devSaved = await ev(`(async () => {
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('保存')).click();
    for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 1500));
    const n = window.__engramDebug?.snapshot?.nodes?.find(x => x.id === 'vfy-dev-1');
    return { title: n?.title, bodyChars: (n?.body || '').length, tags: Object.values(n?.tags || {}), rev: n?.revision, msg: document.querySelector('.rd-msg')?.textContent?.trim() || null };
  })()`);
  R.devSavedFile = readFile(WS_DEV, 'vfy-dev-1');
  await shot('v20-4-saved-dev');

  // 开发模式：新建第二个节点当新父 → 把 vfy-dev-1 改挂过去
  R.devReparent = await ev(`(async () => {
    document.querySelector('.rt-tool.primary').click();
    await new Promise(r => setTimeout(r, 300));
    const setV = (id, v) => { const el = document.getElementById(id); el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); };
    setV('n-title', '新的父节点');
    setV('n-id', 'vfy-parent-1');
    setV('n-body', '父节点正文');
    [...document.querySelectorAll('.parent-clear')].find(b => b.textContent.includes('独立节点'))?.click();
    await new Promise(r => setTimeout(r, 150));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
    for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 800));
    document.getElementById('rt-vfy-dev-1')?.click();
    await new Promise(r => setTimeout(r, 400));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('编辑')).click();
    await new Promise(r => setTimeout(r, 300));
    const search = document.querySelector('.rd-form .parent-search');
    search.value = 'vfy-parent-1';
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise(r => setTimeout(r, 250));
    const optText = document.querySelector('.parent-opt')?.textContent?.trim() || null;
    document.querySelector('.parent-opt')?.click();
    await new Promise(r => setTimeout(r, 150));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('保存')).click();
    for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 1200));
    const s = window.__engramDebug?.snapshot;
    return { optText, parentNow: s?.nodes?.find(n => n.id === 'vfy-dev-1')?.parent ?? null, edges: s?.edges?.length };
  })()`);
  await shot('v20-5-reparent-dev');

  R.devDelete = await ev(`(async () => {
    // 删 vfy-dev-1（现在是 vfy-parent-1 的子节点，开发模式自由删）
    document.getElementById('rt-vfy-dev-1')?.click();
    await new Promise(r => setTimeout(r, 350));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除')).click();
    await new Promise(r => setTimeout(r, 200));
    const armed = [...document.querySelectorAll('.rd-btn')].some(b => b.textContent.includes('确认删除'));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除')).click();
    await new Promise(r => setTimeout(r, 1200));
    const s = window.__engramDebug?.snapshot;
    return { twoStep: armed, gone: !s?.nodes?.some(n => n.id === 'vfy-dev-1'), nodes: s?.nodes?.length };
  })()`);

  R.devCleanup = await ev(`(async () => {
    const row = document.getElementById('rt-vfy-parent-1');
    row?.click(); await new Promise(r => setTimeout(r, 350));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
    await new Promise(r => setTimeout(r, 200));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
    await new Promise(r => setTimeout(r, 1000));
    const s = window.__engramDebug?.snapshot;
    return { nodes: s?.nodes?.length, leftover: s?.nodes?.filter(n => n.id.startsWith('vfy-')).map(n => n.id) };
  })()`);
  R.devExit = await exitTree();

  // ── B. 分析模式（护栏真测）──────────────────────────────────────────────
  R.anaOpen = await openWs('ws_ana', '分析');
  R.anaEnter = await enterTree();

  R.anaNewDefaults = await ev(`(async () => {
    const s = window.__engramDebug?.snapshot;
    const rootGoal = s?.nodes?.find(n => n.parent === null)?.id;
    document.getElementById('rt-' + rootGoal)?.click();
    await new Promise(r => setTimeout(r, 350));
    document.querySelector('.rt-tool.primary').click();
    await new Promise(r => setTimeout(r, 300));
    return {
      rootGoal,
      suggestedId: document.getElementById('n-id')?.value,
      note: document.querySelector('.rd-hint')?.textContent?.replace(/\\s+/g, ' ').trim().slice(0, 60),
      types: [...document.querySelectorAll('#n-type option')].map(o => o.value),
      statuses: [...document.querySelectorAll('#n-status option')].map(o => o.value),
      hasDetach: [...document.querySelectorAll('.parent-clear')].length,
    };
  })()`);
  await shot('v20-6-new-analysis');

  R.anaCreate = await ev(`(async () => {
    const cyBefore = window.__engramDebug?.cy?.nodes().length ?? -1;
    const setV = (id, v) => { const el = document.getElementById(id); el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); };
    const ts = document.getElementById('n-type'); ts.value = 'design'; ts.dispatchEvent(new Event('change', { bubbles: true }));
    await new Promise(r => setTimeout(r, 150));
    const idAfterType = document.getElementById('n-id')?.value;
    const ss = document.getElementById('n-status'); ss.value = 'in_progress'; ss.dispatchEvent(new Event('change', { bubbles: true }));
    setV('n-title', '分析模式新建实测');
    setV('n-body', '# 分析模式新建实测\\n\\n验证分析模式人用通道：新建 → 进链 + 图谱同一父节点下长出。');
    setV('n-tags', '实测');
    await new Promise(r => setTimeout(r, 200));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
    for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 1500));
    const s = window.__engramDebug?.snapshot;
    const n = s?.nodes?.find(x => x.title === '分析模式新建实测');
    const cy = window.__engramDebug?.cy;
    return {
      idAfterType, newId: n?.id, parent: n?.parent, type: n?.type, status: n?.status,
      bodyChars: (n?.body || '').length, tags: Object.values(n?.tags || {}), rev: n?.revision,
      graphBefore: cyBefore, graphAfter: cy ? cy.nodes().length : -1,
      graphHasNode: n && cy ? cy.getElementById(n.id).nonempty() : false,
      treeRow: n ? !!document.getElementById('rt-' + n.id) : false,
      validation: s?.validation?.valid, errors: s?.validation?.errors?.slice(0, 3),
    };
  })()`);
  R.anaCreatedFile = readFile(WS_ANA, R.anaCreate?.newId || 'x');
  await shot('v20-7-created-analysis');

  R.anaGuards = await ev(`(async () => {
    const out = {};
    const s = window.__engramDebug?.snapshot;
    const rootGoal = s?.nodes?.find(n => n.parent === null)?.id;
    const newId = s?.nodes?.find(n => n.title === '分析模式新建实测')?.id;
    // ① 删根 → 拒绝
    document.getElementById('rt-' + rootGoal)?.click();
    await new Promise(r => setTimeout(r, 350));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
    await new Promise(r => setTimeout(r, 200));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
    await new Promise(r => setTimeout(r, 900));
    out.deleteRootError = document.querySelector('.rd-formerr')?.textContent?.trim() || null;
    out.rootStillThere = !!window.__engramDebug?.snapshot?.nodes?.some(n => n.id === rootGoal);
    // ② 新建一个子节点，然后删有子节点的父 → 拒绝
    document.getElementById('rt-' + newId)?.click();
    await new Promise(r => setTimeout(r, 300));
    document.querySelector('.rt-tool.primary').click();
    await new Promise(r => setTimeout(r, 300));
    const setV = (id, v) => { const el = document.getElementById(id); el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); };
    setV('n-title', '子节点（护栏测试）');
    setV('n-body', '子节点正文');
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
    for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 900));
    document.getElementById('rt-' + newId)?.click();
    await new Promise(r => setTimeout(r, 350));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
    await new Promise(r => setTimeout(r, 200));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
    await new Promise(r => setTimeout(r, 900));
    out.deleteWithChildrenError = document.querySelector('.rd-formerr')?.textContent?.trim() || null;
    out.parentStillThere = !!window.__engramDebug?.snapshot?.nodes?.some(n => n.id === newId);
    return out;
  })()`);
  await shot('v20-8-guards-analysis');

  R.anaCycle = await ev(`(async () => {
    const s0 = window.__engramDebug?.snapshot;
    const rootGoal = s0?.nodes?.find(n => n.parent === null)?.id;
    const parentId = s0?.nodes?.find(n => n.title === '分析模式新建实测')?.id;
    // 把根 goal 挂到它的后代（parentId）下 → 应被环护栏拒绝
    document.getElementById('rt-' + rootGoal)?.click();
    await new Promise(r => setTimeout(r, 350));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('编辑')).click();
    await new Promise(r => setTimeout(r, 300));
    const search = document.querySelector('.rd-form .parent-search');
    search.value = parentId;
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise(r => setTimeout(r, 300));
    const opt = document.querySelector('.parent-opt');
    const optText = opt?.textContent?.trim() || null;
    opt?.click();
    await new Promise(r => setTimeout(r, 150));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('保存')).click();
    await new Promise(r => setTimeout(r, 1200));
    const s = window.__engramDebug?.snapshot;
    return {
      optText,
      cycleError: document.querySelector('.rd-formerr')?.textContent?.trim() || null,
      rootParentUnchanged: s?.nodes?.find(n => n.id === rootGoal)?.parent ?? 'null-expected',
      stillEditing: !!document.querySelector('.rd-form'),
    };
  })()`);
  await shot('v20-9-cycle-guard');
  await ev(`(async () => { [...document.querySelectorAll('.rd-btn')].find(x => x.textContent.includes('取消'))?.click(); await new Promise(r => setTimeout(r, 300)); return true; })()`);

  // 清理分析模式测试节点（叶子可删）
  R.anaCleanup = await ev(`(async () => {
    const s = window.__engramDebug?.snapshot;
    const ids = s?.nodes?.filter(n => n.title.startsWith('子节点（护栏测试）')).map(n => n.id) || [];
    const parent = s?.nodes?.find(n => n.title === '分析模式新建实测')?.id;
    for (const id of [...ids, parent].filter(Boolean)) {
      document.getElementById('rt-' + id)?.click();
      await new Promise(r => setTimeout(r, 350));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
      await new Promise(r => setTimeout(r, 200));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
      await new Promise(r => setTimeout(r, 900));
    }
    const s2 = window.__engramDebug?.snapshot;
    return { nodes: s2?.nodes?.length, leftover: s2?.nodes?.filter(n => n.title.includes('实测') || n.title.includes('护栏')).map(n => n.id), validation: s2?.validation?.valid };
  })()`);
  R.anaExit = await exitTree();

  // 移除临时工作区
  R.removeWs = await ev(`(async () => {
    const out = [];
    for (const dir of [${JSON.stringify(WS_DEV)}, ${JSON.stringify(WS_ANA)}]) {
      try { await window.__TAURI_INTERNALS__.invoke('remove_workspace', { dir }); out.push({ dir, ok: true }); }
      catch (e) { out.push({ dir, ok: false, err: String(e) }); }
    }
    return out;
  })()`);

  R.exceptions = exceptions;
  R.consoleErrors = consoleErrors;
  console.log(JSON.stringify(R, null, 2));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL:', e.message); console.log(JSON.stringify(R, null, 2)); process.exit(1); });
