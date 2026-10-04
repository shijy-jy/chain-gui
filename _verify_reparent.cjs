// 终验 2：改挂载（reparent）+ 成环护栏 + 删有子节点护栏（假设修正版）
const http = require('http');
const fs = require('fs');
const path = require('path');
const PORT = process.env.CDP_PORT || '9225';
const WS_ANA = 'G:\\test1.x\\_scratch\\ws_ana';
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
const R = {};
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
  const shot = async (name) => {
    const r = await cdp(ws, 'Page.captureScreenshot', { format: 'png' });
    fs.writeFileSync(path.join(__dirname, '_shots', name + '.png'), Buffer.from(r.data, 'base64'));
  };
  const readFile = (root, id) => {
    const p = path.join(root, '.chain', 'nodes', `${id}.md`);
    return fs.existsSync(p) ? fs.readFileSync(p, 'utf8') : '(missing)';
  };
  const waitReady = async () => { for (let i = 0; i < 120; i++) { if (await ev(`!!document.querySelector('.ws-row') && !!window.__engramDebug?.snapshot`)) return true; await sleep(250); } return false; };
  const reloadInto = async (dir, mode) => {
    await ev(`(() => { localStorage.setItem('chain-gui-last-dir', ${JSON.stringify(dir)}); localStorage.setItem('chain-gui-mode', ${JSON.stringify(mode)}); return true; })()`);
    await cdp(ws, 'Page.reload', {});
    await sleep(1800);
    await waitReady();
    return ev(`(() => ({ dir: (document.querySelector('.dir')?.textContent || '').trim(), mode: window.__engramDebug?.mode, nodes: window.__engramDebug?.snapshot?.nodes?.length }))()`);
  };
  const enterTree = () => ev(`(async () => {
    const b = document.querySelector('.read-toggle'); if (b && b.textContent.includes('文件树')) b.click();
    for (let i = 0; i < 300; i++) { if (document.querySelector('.rm-mask')) break; await new Promise(r => setTimeout(r, 10)); }
    await new Promise(r => requestAnimationFrame(r)); return !!document.querySelector('.rm-mask');
  })()`);
  const selectRow = (id) => ev(`(async () => { document.getElementById('rt-' + ${JSON.stringify(id)})?.click(); await new Promise(r => setTimeout(r, 350));
    return (document.querySelector('.rd-meta .chip.id')?.textContent || '').trim(); })()`);
  const newChild = (id, title, type) => ev(`(async () => {
    document.querySelector('.rt-tool.primary').click();
    await new Promise(r => setTimeout(r, 300));
    const setV = (i, v) => { const el = document.getElementById(i); if (el) { el.value = v; el.dispatchEvent(new Event('input', { bubbles: true })); } };
    ${'const ts = document.getElementById(\'n-type\'); if (ts && ' + JSON.stringify(type || '') + ') { ts.value = ' + JSON.stringify(type || 'task') + '; ts.dispatchEvent(new Event(\'change\', { bubbles: true })); await new Promise(r => setTimeout(r, 150)); }'}
    setV('n-title', ${JSON.stringify(title)});
    setV('n-id', ${JSON.stringify(id)});
    setV('n-body', '正文：' + ${JSON.stringify(title)});
    await new Promise(r => setTimeout(r, 150));
    const parentPlaceholder = document.querySelector('.rd-form .parent-search')?.placeholder || null;
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('创建节点')).click();
    for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 1000));
    const n = window.__engramDebug?.snapshot?.nodes?.find(x => x.id === ${JSON.stringify(id)});
    return { created: !!n, parent: n?.parent ?? null, parentPlaceholder, err: document.querySelector('.rd-formerr')?.textContent?.trim() || null };
  })()`);
  const reparent = (query) => ev(`(async () => {
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('编辑')).click();
    await new Promise(r => setTimeout(r, 350));
    const s = document.querySelector('.rd-form .parent-search');
    s.focus();
    await new Promise(r => setTimeout(r, 120));
    s.value = ${JSON.stringify(query)};
    s.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise(r => setTimeout(r, 350));
    const options = [...document.querySelectorAll('.parent-opt')].map(o => o.textContent.trim());
    document.querySelector('.parent-opt')?.click();
    await new Promise(r => setTimeout(r, 200));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('保存')).click();
    for (let i = 0; i < 300; i++) { if (!document.querySelector('.rd-form')) break; await new Promise(r => setTimeout(r, 60)); }
    await new Promise(r => setTimeout(r, 1200));
    return { options, err: document.querySelector('.rd-formerr')?.textContent?.trim() || null, stillEditing: !!document.querySelector('.rd-form') };
  })()`);
  const cancel = () => ev(`(async () => { [...document.querySelectorAll('.rd-btn')].find(x => x.textContent.includes('取消'))?.click(); await new Promise(r => setTimeout(r, 300)); return !document.querySelector('.rd-form'); })()`);

  await waitReady();
  await ev(`(async () => { for (const [dir, mode] of [[${JSON.stringify(WS_DEV)}, 'dev'], [${JSON.stringify(WS_ANA)}, 'analysis']]) { try { await window.__TAURI_INTERNALS__.invoke('add_workspace', { dir, mode }); } catch (e) {} } return true; })()`);

  // ── A. 开发模式：两个独立节点 → 把 A 挂到 B 下 ────────────────────────
  R.devOpen = await reloadInto(WS_DEV, 'dev');
  await enterTree();
  R.devA = await newChild('rp-a', '节点A', null);   // 默认挂当前选中（g-001）
  R.devB = await newChild('rp-b', '节点B', null);
  R.devSelectA = await selectRow('rp-a');
  R.devReparent = await reparent('rp-b');
  R.devAfter = await ev(`(() => { const s = window.__engramDebug?.snapshot; const a = s?.nodes?.find(n => n.id === 'rp-a');
    const e = s?.edges?.find(x => x.child === 'rp-a');
    return { aParent: a?.parent ?? null, edge: e ? (e.parent + '->' + e.child) : null, treeNested: !!document.querySelector('#rt-rp-b') && !!document.querySelector('#rt-rp-a') }; })()`);
  R.devFile = readFile(WS_DEV, 'rp-a').split('\n').filter((l) => l.startsWith('parent:') || l.startsWith('rel:')).join(' | ');
  await shot('v22-1-reparent-dev');

  // ── B. 分析模式：父子三层 → 护栏 ──────────────────────────────────────
  R.anaOpen = await reloadInto(WS_ANA, 'analysis');
  await enterTree();
  R.anaD = await newChild('d-001', '设计层', 'design');       // 默认挂在选中的 g-001 下
  R.anaT = await newChild('t-001', '任务层', 'task');         // 默认挂在刚创建的 d-001 下？
  R.anaTEdges = await ev(`(() => { const s = window.__engramDebug?.snapshot; return s?.edges?.map(e => e.parent + '->' + e.child); })()`);

  // 删 d-001（有子节点 t-001）→ 应被拒
  R.anaSelectD = await selectRow('d-001');
  R.anaDeleteParent = await ev(`(async () => {
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
    await new Promise(r => setTimeout(r, 200));
    [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
    await new Promise(r => setTimeout(r, 900));
    return { err: document.querySelector('.rd-formerr')?.textContent?.trim() || null, dAlive: !!window.__engramDebug?.snapshot?.nodes?.some(n => n.id === 'd-001') };
  })()`);
  await shot('v22-2-children-guard');

  // 成环：把 g-001 挂到它的后代 d-001 下 → 应被拒
  R.anaSelectRoot = await selectRow('g-001');
  R.anaCycle = await reparent('d-001');
  R.anaCycleState = await ev(`(() => { const s = window.__engramDebug?.snapshot; return { rootParent: s?.nodes?.find(n => n.id === 'g-001')?.parent ?? null, valid: s?.validation?.valid }; })()`);
  await shot('v22-3-cycle-guard');
  await cancel();

  // 合法改挂：t-001 → g-001
  R.anaSelectT = await selectRow('t-001');
  R.anaReparentOk = await reparent('g-001');
  R.anaAfterMove = await ev(`(() => { const s = window.__engramDebug?.snapshot; const t = s?.nodes?.find(n => n.id === 't-001');
    return { tParent: t?.parent ?? null, edges: s?.edges?.map(e => e.parent + '->' + e.child), valid: s?.validation?.valid }; })()`);
  R.anaFile = readFile(WS_ANA, 't-001').split('\n').filter((l) => l.startsWith('parent:') || l.startsWith('rel:')).join(' | ');
  await shot('v22-4-reparent-analysis');

  // 清场（叶子优先）
  R.anaCleanup = await ev(`(async () => {
    for (const id of ['t-001', 'd-001']) {
      const row = document.getElementById('rt-' + id);
      row?.click(); await new Promise(r => setTimeout(r, 350));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
      await new Promise(r => setTimeout(r, 200));
      [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
      await new Promise(r => setTimeout(r, 900));
    }
    const s = window.__engramDebug?.snapshot;
    return { nodes: s?.nodes?.length, leftover: s?.nodes?.map(n => n.id), valid: s?.validation?.valid };
  })()`);

  R.devCleanup = await (async () => {
    await reloadInto(WS_DEV, 'dev');
    await enterTree();
    return ev(`(async () => {
      for (const id of ['rp-a', 'rp-b']) {
        const row = document.getElementById('rt-' + id);
        row?.click(); await new Promise(r => setTimeout(r, 350));
        [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('删除'))?.click();
        await new Promise(r => setTimeout(r, 200));
        [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('确认删除'))?.click();
        await new Promise(r => setTimeout(r, 900));
      }
      const s = window.__engramDebug?.snapshot;
      return { nodes: s?.nodes?.length, leftover: s?.nodes?.map(n => n.id) };
    })()`);
  })();

  R.removeWs = await ev(`(async () => { const out = []; for (const dir of [${JSON.stringify(WS_DEV)}, ${JSON.stringify(WS_ANA)}]) { try { await window.__TAURI_INTERNALS__.invoke('remove_workspace', { dir }); out.push({ dir, ok: true }); } catch (e) { out.push({ dir, ok: false, err: String(e) }); } } return out; })()`);
  console.log(JSON.stringify(R, null, 2));
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL:', e.message); console.log(JSON.stringify(R, null, 2)); process.exit(1); });
