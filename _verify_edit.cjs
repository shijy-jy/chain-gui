// v2.20 文件树模式「新建 + 编辑」实测：真实应用内 CDP 驱动（开发 + 分析模式两套工作区）
// 用法：node _verify_edit.cjs   （应用需带 --remote-debugging-port=9225 启动）
const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = process.env.CDP_PORT || '9225';
const SHOTS = path.join(__dirname, '_shots');
const WS_DEV = 'G:\\test1.x\\_scratch\\ws_dev';
const WS_ANA = 'G:\\test1.x\\_scratch\\ws_ana';

function getJson(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => { let d = ''; res.on('data', (c) => (d += c)); res.on('end', () => resolve(JSON.parse(d))); })
      .on('error', reject);
  });
}
let seq = 0;
const pending = new Map();
function cdp(ws, method, params = {}) {
  return new Promise((resolve, reject) => { const id = ++seq; pending.set(id, { resolve, reject }); ws.send(JSON.stringify({ id, method, params })); });
}

const results = {};
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
    if (d.id && pending.has(d.id)) {
      const p = pending.get(d.id); pending.delete(d.id);
      d.error ? p.reject(new Error(d.error.message)) : p.resolve(d.result);
      return;
    }
    if (d.method === 'Runtime.exceptionThrown') {
      exceptions.push(d.params.exceptionDetails.exception?.description || d.params.exceptionDetails.text);
    }
    if (d.method === 'Runtime.consoleAPICalled' && d.params.type === 'error') {
      consoleErrors.push((d.params.args || []).map((a) => a.value ?? a.description).join(' '));
    }
  };
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws error')); });
  await cdp(ws, 'Runtime.enable');
  await cdp(ws, 'Page.enable');

  const ev = async (expression, awaitPromise = true) => {
    const r = await cdp(ws, 'Runtime.evaluate', { expression, awaitPromise, returnByValue: true });
    if (r.exceptionDetails) throw new Error('eval error: ' + r.exceptionDetails.text + ' ' + (r.exceptionDetails.exception?.description || ''));
    return r.result.value;
  };
  const shot = async (name) => {
    const r = await cdp(ws, 'Page.captureScreenshot', { format: 'png' });
    fs.writeFileSync(path.join(SHOTS, name + '.png'), Buffer.from(r.data, 'base64'));
  };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  for (let i = 0; i < 100; i++) {
    if (await ev(`!!document.querySelector('.ws-row') || !!document.querySelector('.cy-container')`)) break;
    await sleep(300);
  }
  // 归一：回图谱模式
  await ev(`(async () => { for (let i = 0; i < 5 && document.querySelector('.rm-mask'); i++) { window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await new Promise(r => setTimeout(r, 250)); } return true; })()`);

  // 注册两个临时工作区（开发 + 分析）—— add_workspace 会顺带初始化 .chain
  results.addWorkspaces = await ev(`(async () => {
    const out = [];
    for (const [dir, mode] of ${JSON.stringify([['dev', WS_DEV], ['analysis', WS_ANA]])}) {
      try {
        const list = await window.__TAURI_INTERNALS__.invoke('add_workspace', { dir, mode });
        out.push({ mode, ok: true, count: list.length });
      } catch (e) { out.push({ mode, ok: false, err: String(e) }); }
    }
    return out;
  })()`);

  const openWs = async (name) => ev(`(async () => {
    const tab = [...document.querySelectorAll('.mode-tab')];
    const rows = [...document.querySelectorAll('.ws-row')];
    const row = rows.find(r => (r.querySelector('.ws-name')?.textContent || '').trim() === ${JSON.stringify('__NAME__')});
    if (!row) return { ok: false, names: rows.map(r => r.querySelector('.ws-name')?.textContent) };
    const want = row.getAttribute('title') || '';
    row.click();
    for (let i = 0; i < 200; i++) {
      const s = window.__engramDebug?.snapshot;
      if (s && (document.querySelector('.dir')?.textContent || '').trim() === want) break;
      await new Promise(r => setTimeout(r, 120));
    }
    await new Promise(r => setTimeout(r, 600));
    const s = window.__engramDebug?.snapshot;
    return { ok: true, dir: (document.querySelector('.dir')?.textContent || '').trim(), nodes: s?.nodes?.length ?? -1, mode: window.__engramDebug?.mode };
  })()`.replace('__NAME__', name));

  const enterTree = async () => ev(`(async () => {
    const btn = document.querySelector('.read-toggle');
    if (!btn || btn.textContent.includes('文件树')) btn?.click();
    for (let i = 0; i < 300; i++) { if (document.querySelector('.rm-mask')) break; await new Promise(r => setTimeout(r, 10)); }
    await new Promise(r => requestAnimationFrame(r));
    return { overlay: !!document.querySelector('.rm-mask'), badge: document.querySelector('.rm-badge')?.textContent?.trim(), hasNewBtn: !!document.querySelector('.rt-tool.primary') };
  })()`);

  const exitTree = async () => ev(`(async () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await new Promise(r => setTimeout(r, 300));
    return !document.querySelector('.rm-mask');
  })()`);

  // ── A. 开发模式：新建 → 编辑 → 删除 ────────────────────────────────────
  const devTab = await ev(`(async () => {
    const t = [...document.querySelectorAll('.mode-tab')].find(x => x.textContent.includes('开发'));
    t.click(); await new Promise(r => setTimeout(r, 500)); return true;
  })()`);
  results.devOpen = await openWs('ws_dev');
  results.devEnter = await enterTree();

  results.devCreateForm = await ev(`(async () => {
    document.querySelector('.rt-tool.primary').click();
    await new Promise(r => setTimeout(r, 200));
    const inputs = [...document.querySelectorAll('.rd-form input, .rd-form select')].map(el => el.id || el.className);
    const setV = (id, v) => { const el = document.getElementById(id); el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); };
    setV('n-title', '驱动实测新建节点');
    setV('n-id', 'test-new-1');
    setV('n-tags', '实测, 临时');
    setV('n-body', '# 驱动实测新建节点\\n\\n这是 CDP 自动化写入的正文，用来验证「文件树新建 → 图谱同步长出节点」。');
    await new Promise(r => setTimeout(r, 150));
    return { fields: inputs, title: document.getElementById('n-title')?.value, id: document.getElementById('n-id')?.value, parentHint: document.querySelector('.rd-form .fld span')?.textContent?.trim() };
  })()`);
  await sleep(200);
  await shot('edit-1-new-form-dev');

  results.devCreate = await ev(`(async () => {
    const beforeTree = document.querySelectorAll('.trow').length;
    const cyBefore = window.__engramDebug?.cy?.nodes().length ?? -1;
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
    for (let i = 0; i < 200; i++) {
      if (!document.querySelector('.rd-form')) break;
      await new Promise(r => setTimeout(r, 60));
    }
    await new Promise(r => setTimeout(r, 500));
    const cy = window.__engramDebug?.cy;
    const cyAfter = cy ? cy.nodes().length : -1;
    const node = window.__engramDebug?.snapshot?.nodes?.find(n => n.id === 'test-new-1');
    return {
      inSnapshot: !!node,
      title: node?.title,
      tags: node?.tags,
      bodyChars: (node?.body || '').length,
      parent: node?.parent,
      type: node?.type,
      status: node?.status,
      graphNodesBefore: cyBefore,
      graphNodesAfter: cyAfter,
      graphHasNode: cy ? cy.getElementById('test-new-1').nonempty() : false,
      treeHasRow: !!document.getElementById('rt-test-new-1'),
      readerShowsIt: (document.querySelector('.rd-meta .chip.id')?.textContent || '').trim() === 'test-new-1',
      revisions: node?.revision,
    };
  })()`);
  await sleep(300);
  await shot('edit-2-created-dev');

  results.devEdit = await ev(`(async () => {
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('编辑')).click();
    await new Promise(r => setTimeout(r, 250));
    const form = !!document.querySelector('.rd-form');
    const t = document.getElementById('f-title');
    t.value = '驱动实测·改过标题';
    t.dispatchEvent(new Event('input', { bubbles: true }));
    const b = document.getElementById('f-body');
    b.value = '# 改过的标题\\n\\n正文也被改写（第二版）。';
    b.dispatchEvent(new Event('input', { bubbles: true }));
    const g = document.getElementById('f-tags');
    g.value = '实测, 已编辑';
    g.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise(r => setTimeout(r, 120));
    return { form, oldTitle: '驱动实测新建节点' };
  })()`);
  await sleep(200);
  await shot('edit-3-edit-form-dev');

  results.devSaved = await ev(`(async () => {
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('保存')).click();
    for (let i = 0; i < 200; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 400));
    const node = window.__engramDebug?.snapshot?.nodes?.find(n => n.id === 'test-new-1');
    return {
      title: node?.title, tags: node?.tags, bodyHasNewText: (node?.body || '').includes('第二版'),
      revision: node?.revision, msg: document.querySelector('.rd-msg')?.textContent?.trim() || null,
      readerTitle: document.querySelector('.rd-crumb.cur')?.textContent?.trim() || null,
    };
  })()`);
  await sleep(200);
  await shot('edit-4-saved-dev');

  // 开发模式：改挂载位置（新建一个父节点，把 test-new-1 挂过去）
  results.devReparent = await ev(`(async () => {
    // 先新建一个独立节点当新父
    document.querySelector('.rt-tool.primary').click();
    await new Promise(r => setTimeout(r, 200));
    const setV = (id, v) => { const el = document.getElementById(id); el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); };
    setV('n-title', '新的父节点');
    setV('n-id', 'test-parent-1');
    setV('n-body', '父节点正文');
    const indep = [...document.querySelectorAll('.parent-clear')].find(b => b.textContent.includes('独立节点'));
    if (indep) indep.click();
    await new Promise(r => setTimeout(r, 120));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
    for (let i = 0; i < 200; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 400));
    const created = window.__engramDebug?.snapshot?.nodes?.find(n => n.id === 'test-parent-1');
    // 回到 test-new-1，编辑 → 改父节点
    const row = document.getElementById('rt-test-new-1');
    if (row) row.click();
    await new Promise(r => setTimeout(r, 300));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('编辑')).click();
    await new Promise(r => setTimeout(r, 250));
    const search = document.querySelector('.rd-form .parent-search');
    search.value = 'test-parent-1';
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise(r => setTimeout(r, 200));
    const opt = document.querySelector('.parent-opt');
    const optText = opt?.textContent?.trim() || null;
    opt?.click();
    await new Promise(r => setTimeout(r, 150));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('保存')).click();
    for (let i = 0; i < 200; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 400));
    const node = window.__engramDebug?.snapshot?.nodes?.find(n => n.id === 'test-new-1');
    return { parentCreated: !!created, optText, parentNow: node?.parent, edges: window.__engramDebug?.snapshot?.edges?.length };
  })()`);
  await sleep(300);
  await shot('edit-5-reparent-dev');

  results.devDeleteGuards = await ev(`(async () => {
    // 删父节点：它现在有一个子节点 → 开发模式允许（自由图谱），确认两段式交互即可
    const row = document.getElementById('rt-test-parent-1');
    row?.click();
    await new Promise(r => setTimeout(r, 300));
    const delBtn = [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'));
    delBtn.click();
    await new Promise(r => setTimeout(r, 200));
    const armed = [...document.querySelectorAll('.rd-btn')].some(b => b.textContent.includes('确认删除'));
    // 再点一次 = 真删
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除')).click();
    for (let i = 0; i < 200; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 80)); }
    await new Promise(r => setTimeout(r, 500));
    const s = window.__engramDebug?.snapshot;
    return {
      twoStep: armed,
      parentGone: !s?.nodes?.some(n => n.id === 'test-parent-1'),
      childStillThere: !!s?.nodes?.some(n => n.id === 'test-new-1'),
      childParentNow: s?.nodes?.find(n => n.id === 'test-new-1')?.parent ?? null,
      nodes: s?.nodes?.length,
    };
  })()`);
  await sleep(300);
  await shot('edit-6-deleted-dev');

  // 清理开发模式测试节点
  results.devCleanup = await ev(`(async () => {
    for (const id of ['test-new-1']) {
      try {
        const row = document.getElementById('rt-' + id);
        row?.click();
        await new Promise(r => setTimeout(r, 250));
        const del = [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'));
        del?.click(); await new Promise(r => setTimeout(r, 150));
        [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
        await new Promise(r => setTimeout(r, 400));
      } catch (e) {}
    }
    const s = window.__engramDebug?.snapshot;
    return { nodes: s?.nodes?.length, leftover: s?.nodes?.filter(n => n.id.startsWith('test-')).map(n => n.id) };
  })()`);
  await exitTree();

  // ── B. 分析模式：新建（必挂父）/ 编辑 / 护栏 ────────────────────────────
  await ev(`(async () => { const t = [...document.querySelectorAll('.mode-tab')].find(x => x.textContent.includes('分析')); t.click(); await new Promise(r => setTimeout(r, 500)); return true; })()`);
  results.anaOpen = await openWs('ws_ana');
  results.anaEnter = await enterTree();

  results.anaNewDefaults = await ev(`(async () => {
    // 选中根 goal 后点新建：默认父节点应为当前节点，id 建议按协议前缀
    const rootId = window.__engramDebug?.snapshot?.manifest?.root;
    document.getElementById('rt-' + rootId)?.click();
    await new Promise(r => setTimeout(r, 250));
    document.querySelector('.rt-tool.primary').click();
    await new Promise(r => setTimeout(r, 250));
    return {
      rootId,
      suggestedId: document.getElementById('n-id')?.value,
      formNote: document.querySelector('.rd-hint')?.textContent?.replace(/\\s+/g, ' ').trim().slice(0, 80),
      typeOptions: [...document.querySelectorAll('#n-type option')].map(o => o.value),
      statusOptions: [...document.querySelectorAll('#n-status option')].map(o => o.value),
    };
  })()`);
  await sleep(200);
  await shot('edit-7-new-form-analysis');

  results.anaCreate = await ev(`(async () => {
    const cyBefore = window.__engramDebug?.cy?.nodes().length ?? -1;
    const setV = (id, v) => { const el = document.getElementById(id); el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); };
    const rootId = window.__engramDebug?.snapshot?.manifest?.root;
    // 显式选「设计」类型：验证前端 → 后端 node_type 字段名与协议词表校验都通
    const typeSel = document.getElementById('n-type');
    typeSel.value = 'design';
    typeSel.dispatchEvent(new Event('change', { bubbles: true }));
    await new Promise(r => setTimeout(r, 120));
    const idAfterType = document.getElementById('n-id')?.value;
    const statusSel = document.getElementById('n-status');
    statusSel.value = 'in_progress';
    statusSel.dispatchEvent(new Event('change', { bubbles: true }));
    setV('n-title', '驱动实测·分析模式设计节点');
    setV('n-body', '设计目标：验证分析模式人在文件树里新建节点后，节点同时进链与图谱。');
    setV('n-tags', '实测');
    await new Promise(r => setTimeout(r, 150));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
    for (let i = 0; i < 250; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 600));
    const s = window.__engramDebug?.snapshot;
    const mine = s?.nodes?.filter(n => n.title.includes('驱动实测·分析模式'));
    const node = mine?.[0];
    const cy = window.__engramDebug?.cy;
    return {
      idAfterType,
      newNodeId: node?.id,
      parent: node?.parent,
      rootId,
      type: node?.type,
      status: node?.status,
      revisions: node?.revision,
      bodyChars: (node?.body || '').length,
      tags: node?.tags,
      cyBefore, cyAfter: cy ? cy.nodes().length : -1,
      graphHasNode: node && cy ? cy.getElementById(node.id).nonempty() : false,
      treeRowHighlighted: node ? !!document.getElementById('rt-' + node.id) : false,
      validation: s?.validation?.valid,
      validationErrors: s?.validation?.errors?.slice(0, 3),
    };
  })()`);
  await sleep(300);
  await shot('edit-8-created-analysis');

  results.anaGuards = await ev(`(async () => {
    const rootId = window.__engramDebug?.snapshot?.manifest?.root;
    const newId = window.__engramDebug?.snapshot?.nodes?.find(n => n.title.includes('驱动实测·分析模式'))?.id;
    const out = {};
    // ① 删根 → 护栏拒绝
    document.getElementById('rt-' + rootId)?.click();
    await new Promise(r => setTimeout(r, 250));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
    await new Promise(r => setTimeout(r, 150));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
    await new Promise(r => setTimeout(r, 500));
    out.deleteRootError = document.querySelector('.rd-formerr')?.textContent?.trim() || null;
    // ② 断开父节点（编辑里没有「断开」按钮 → 应不存在）
    document.getElementById('rt-' + newId)?.click();
    await new Promise(r => setTimeout(r, 300));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('编辑'))?.click();
    await new Promise(r => setTimeout(r, 250));
    out.hasDetachButton = [...document.querySelectorAll('.parent-clear')].length > 0;
    // ③ 空正文保存 → 客户端拦截
    const b = document.getElementById('f-body');
    b.value = '   ';
    b.dispatchEvent(new Event('input', { bubbles: true }));
    [...document.querySelectorAll('.rd-btn')].find(x => x.textContent.includes('保存'))?.click();
    await new Promise(r => setTimeout(r, 350));
    out.emptyBodyError = document.querySelector('.rd-formerr')?.textContent?.trim() || null;
    // ④ 取消 → 回到阅读
    [...document.querySelectorAll('.rd-btn')].find(x => x.textContent.includes('取消'))?.click();
    await new Promise(r => setTimeout(r, 300));
    out.backToRead = !document.querySelector('.rd-form');
    return out;
  })()`);
  await sleep(200);
  await shot('edit-9-guards-analysis');

  results.anaCycle = await ev(`(async () => {
    // 先把新节点挂到根下（本来是挂在根下的）；再尝试让根挂到新节点下 → 成环护栏
    const rootId = window.__engramDebug?.snapshot?.manifest?.root;
    const newId = window.__engramDebug?.snapshot?.nodes?.find(n => n.title.includes('驱动实测·分析模式'))?.id;
    document.getElementById('rt-' + rootId)?.click();
    await new Promise(r => setTimeout(r, 300));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('编辑'))?.click();
    await new Promise(r => setTimeout(r, 250));
    const search = document.querySelector('.rd-form .parent-search');
    search.value = newId;
    search.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise(r => setTimeout(r, 250));
    document.querySelector('.parent-opt')?.click();
    await new Promise(r => setTimeout(r, 150));
    [...document.querySelectorAll('.rd-btn')].find(x => x.textContent.includes('保存'))?.click();
    await new Promise(r => setTimeout(r, 700));
    return {
      cycleError: document.querySelector('.rd-formerr')?.textContent?.trim() || null,
      stillEditing: !!document.querySelector('.rd-form'),
    };
  })()`);
  await sleep(200);
  await shot('edit-10-cycle-guard');

  // ⑤ 删叶子（分析模式允许）
  results.anaDeleteLeaf = await ev(`(async () => {
    [...document.querySelectorAll('.rd-btn')].find(x => x.textContent.includes('取消'))?.click();
    await new Promise(r => setTimeout(r, 250));
    const newId = window.__engramDebug?.snapshot?.nodes?.find(n => n.title.includes('驱动实测·分析模式'))?.id;
    document.getElementById('rt-' + newId)?.click();
    await new Promise(r => setTimeout(r, 300));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
    await new Promise(r => setTimeout(r, 150));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
    await new Promise(r => setTimeout(r, 700));
    const s = window.__engramDebug?.snapshot;
    return {
      gone: !s?.nodes?.some(n => n.id === newId),
      nodes: s?.nodes?.length,
      validation: s?.validation?.valid,
      readerId: document.querySelector('.rd-meta .chip.id')?.textContent?.trim() || null,
    };
  })()`);
  await sleep(300);
  await shot('edit-11-deleted-leaf-analysis');

  results.anaExit = await exitTree();
  results.perfCheck = await ev(`(() => ({ graphNodes: window.__engramDebug?.cy?.nodes().length ?? -1, mode: window.__engramDebug?.mode }))()`);

  // 清理：移除临时工作区 + 删除磁盘目录（在 Node 侧做）
  results.removeWorkspaces = await ev(`(async () => {
    const out = [];
    for (const dir of ${JSON.stringify([WS_DEV, WS_ANA])}) {
      try { await window.__TAURI_INTERNALS__.invoke('remove_workspace', { dir }); out.push({ dir, ok: true }); }
      catch (e) { out.push({ dir, ok: false, err: String(e) }); }
    }
    return out;
  })()`);

  results.exceptions = exceptions;
  results.consoleErrors = consoleErrors;
  console.log(JSON.stringify(results, null, 2));
  ws.close();
  process.exit(0);
})().catch((e) => {
  console.error('FATAL:', e.message);
  console.log(JSON.stringify(results, null, 2));
  process.exit(1);
});
