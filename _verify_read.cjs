// v2.19 阅读模式实测：真实应用内 CDP 驱动（打开工作区 → 切阅读模式 → 交互全流程 → 截图）
// 用法：node _verify_read.cjs   （应用需带 --remote-debugging-port=9223 启动）
const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = process.env.CDP_PORT || '9223';
const SHOTS = path.join(__dirname, '_shots');

function getJson(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => {
      let data = '';
      res.on('data', (c) => (data += c));
      res.on('end', () => resolve(JSON.parse(data)));
    }).on('error', reject);
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

const results = {};
const exceptions = [];
const consoleErrors = [];

(async () => {
  fs.mkdirSync(SHOTS, { recursive: true });
  const targets = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  const page = targets.find((t) => t.type === 'page') || targets[0];
  if (!page) throw new Error('no page target');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  ws.onmessage = (msg) => {
    const d = JSON.parse(msg.data);
    if (d.id && pending.has(d.id)) {
      const p = pending.get(d.id);
      pending.delete(d.id);
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
  await new Promise((res, rej) => {
    ws.onopen = res;
    ws.onerror = () => rej(new Error('ws error'));
  });
  await cdp(ws, 'Runtime.enable');
  await cdp(ws, 'Page.enable');

  const ev = async (expression, awaitPromise = true) => {
    const r = await cdp(ws, 'Runtime.evaluate', { expression, awaitPromise, returnByValue: true });
    if (r.exceptionDetails) {
      throw new Error(
        'eval error: ' +
          r.exceptionDetails.text +
          ' ' +
          (r.exceptionDetails.exception?.description || ''),
      );
    }
    return r.result.value;
  };
  const shot = async (name) => {
    const r = await cdp(ws, 'Page.captureScreenshot', { format: 'png' });
    fs.writeFileSync(path.join(SHOTS, name + '.png'), Buffer.from(r.data, 'base64'));
  };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  // 0) 应用就绪（工作区栏渲染出来）
  for (let i = 0; i < 100; i++) {
    const ready = await ev(`!!document.querySelector('.ws-row') || !!document.querySelector('.cy-container')`);
    if (ready) break;
    await sleep(300);
  }

  // 0.5) 归一化：确保从图谱视图开始（共享 WebView2 profile 时可能残留阅读模式）
  results.normalize = await ev(`(async () => {
    for (let i = 0; i < 5 && document.querySelector('.rm-mask'); i++) {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      await new Promise(r => setTimeout(r, 250));
    }
    return { overlay: !!document.querySelector('.rm-mask'), lsViewMode: localStorage.getItem('engram-view-mode') };
  })()`);

  // 1) 打开分析模式工作区 ta（真实点击左侧栏条目）
  results.openTa = await ev(`(async () => {
    const rows = [...document.querySelectorAll('.ws-row')];
    const row = rows.find(r => (r.querySelector('.ws-name')?.textContent || '').trim() === 'ta');
    if (!row) return { ok: false, names: rows.map(r => r.querySelector('.ws-name')?.textContent) };
    const wantPath = row.getAttribute('title') || '';
    row.click();
    const t0 = performance.now();
    for (let i = 0; i < 200; i++) {
      const s = window.__engramDebug?.snapshot;
      const dirShown = (document.querySelector('.dir')?.textContent || '').trim();
      if (s && s.nodes.length > 0 && dirShown === wantPath) {
        await new Promise(r => requestAnimationFrame(r));
        return { ok: true, ms: Math.round(performance.now() - t0), nodes: s.nodes.length, edges: s.edges.length, root: s.manifest.root, dir: dirShown };
      }
      await new Promise(r => setTimeout(r, 150));
    }
    return { ok: false, why: 'snapshot timeout', wantPath, got: (document.querySelector('.dir')?.textContent || '').trim() };
  })()`);
  await sleep(1200);

  // 2) 人类专属不变量：调试接缝里不得出现阅读模式（AI/CDP 不可识别）
  results.invariants = await ev(`(() => {
    const dbg = window.__engramDebug || {};
    const keys = Object.keys(dbg);
    return {
      debugKeys: keys,
      debugHasReadMode: keys.some(k => /read|reader|阅读/i.test(k)),
      buttonExists: !!document.querySelector('.read-toggle'),
      buttonText: document.querySelector('.read-toggle')?.textContent?.trim() || null,
      overlayBefore: !!document.querySelector('.rm-mask'),
      sourceHasMcp: true,
    };
  })()`);

  // 3) 进入阅读模式（点工具栏按钮，测耗时与首屏渲染）
  results.enter = await ev(`(async () => {
    const btn = document.querySelector('.read-toggle');
    if (!btn) return { ok: false, why: 'no button' };
    const t0 = performance.now();
    btn.click();
    for (let i = 0; i < 300; i++) {
      if (document.querySelector('.rm-mask')) break;
      await new Promise(r => setTimeout(r, 10));
    }
    await new Promise(r => requestAnimationFrame(r));
    const rows = [...document.querySelectorAll('.trow')];
    return {
      ok: !!document.querySelector('.rm-mask'),
      enterMs: Math.round((performance.now() - t0) * 10) / 10,
      visibleRows: rows.length,
      humanChip: document.querySelector('.rm-human')?.textContent?.trim(),
      header: document.querySelector('.rm-badge')?.textContent?.trim(),
      count: document.querySelector('.rm-count')?.textContent?.trim(),
      rootRows: rows.slice(0, 6).map(r => r.textContent.replace(/\\s+/g, ' ').trim()),
      readerTitle: document.querySelector('.rd-crumbs')?.textContent?.replace(/\\s+/g, ' ').trim(),
      readerChips: [...document.querySelectorAll('.rd-meta .chip')].map(c => c.textContent.replace(/\\s+/g, ' ').trim()),
      bodyChars: (document.querySelector('.rd-body')?.textContent || '').length,
      readPos: document.querySelector('.rd-pos')?.textContent?.trim(),
    };
  })()`);
  await sleep(400);
  await shot('read-ta-1-enter');

  // 4) 展开全部 → 行数 / 递归深度 / 交互
  results.expandAll = await ev(`(async () => {
    const t0 = performance.now();
    document.querySelector('.rt-tool')?.click();
    await new Promise(r => requestAnimationFrame(r));
    await new Promise(r => requestAnimationFrame(r));
    return {
      ms: Math.round((performance.now() - t0) * 10) / 10,
      visibleRows: document.querySelectorAll('.trow').length,
      scrollH: document.querySelector('.rt-list')?.scrollHeight,
    };
  })()`);
  await sleep(300);
  await shot('read-ta-2-expand-all');

  // 5) 点一个深层节点（带代码骨架的优先）→ 读全文 + 面包屑
  results.deepSelect = await ev(`(async () => {
    const rows = [...document.querySelectorAll('.trow')];
    const target = rows.find(r => r.querySelector('.tmark.code')) || rows[Math.min(12, rows.length - 1)];
    const before = document.querySelector('.rd-body')?.textContent || '';
    target.click();
    await new Promise(r => setTimeout(r, 250));
    const crumbs = [...document.querySelectorAll('.rd-crumb')].map(c => c.textContent.trim());
    return {
      clicked: target.textContent.replace(/\\s+/g, ' ').trim(),
      crumbCount: crumbs.length,
      crumbs: crumbs.slice(-3),
      bodyChanged: (document.querySelector('.rd-body')?.textContent || '') !== before,
      bodyChars: (document.querySelector('.rd-body')?.textContent || '').length,
      pos: document.querySelector('.rd-pos')?.textContent?.trim(),
      hasCodeBtn: !!document.querySelector('.rd-btn.code'),
      childLinks: document.querySelectorAll('.rd-kid').length,
      evLinks: document.querySelectorAll('.rd-ev-icon').length,
    };
  })()`);
  await sleep(300);
  await shot('read-ta-3-node');

  // 6) 阅读顺序「下一篇」+ 原文/渲染切换 + 字号
  results.navigate = await ev(`(async () => {
    const btns = [...document.querySelectorAll('.rd-btn')];
    const next = btns.find(b => b.textContent.includes('下一篇'));
    const before = document.querySelector('.rd-pos')?.textContent?.trim();
    next.click();
    await new Promise(r => setTimeout(r, 250));
    const after = document.querySelector('.rd-pos')?.textContent?.trim();
    const rawBtn = [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.trim() === '原文');
    rawBtn.click();
    await new Promise(r => setTimeout(r, 200));
    const rawChars = (document.querySelector('.rd-raw')?.textContent || '').length;
    const rendered = !!document.querySelector('.rd-body .md');
    const fontBtn = [...document.querySelectorAll('.rd-btn')].find(b => /^A \\d+px$/.test(b.textContent.trim()));
    const f0 = getComputedStyle(document.querySelector('.rd-body')).fontSize;
    fontBtn.click();
    await new Promise(r => setTimeout(r, 150));
    const f1 = getComputedStyle(document.querySelector('.rd-body')).fontSize;
    return { posBefore: before, posAfter: after, rawChars, renderedWhenRaw: rendered, fontBefore: f0, fontAfter: f1 };
  })()`);
  await sleep(300);
  await shot('read-ta-4-raw');

  // 7) 树内检索（标题/id/标签/正文命中 + 去抖）
  results.search = await ev(`(async () => {
    const input = document.querySelector('.rt-search');
    input.focus();
    input.value = '渲染';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise(r => setTimeout(r, 500));
    const hits = [...document.querySelectorAll('.trow.match')];
    return {
      hits: hits.length,
      first: hits.slice(0, 5).map(h => h.textContent.replace(/\\s+/g, ' ').trim().slice(0, 70)),
      kinds: [...new Set(hits.map(h => h.querySelector('.tkind')?.textContent))],
      clearable: !!document.querySelector('.rt-clear'),
    };
  })()`);
  await sleep(300);
  await shot('read-ta-5-search');

  // 7.5) 「在图谱中定位」：退出阅读模式 → 图上门居中高亮该节点（往返闭环）
  results.locate = await ev(`(async () => {
    const readId = document.querySelector('.rd-meta .chip.id')?.textContent?.trim();
    const btn = [...document.querySelectorAll('.rd-btn')].find(b => b.textContent.includes('在图谱中定位'));
    if (!btn) return { ok: false, why: 'no locate button' };
    btn.click();
    await new Promise(r => setTimeout(r, 700));
    const cy = window.__engramDebug?.cy;
    const el = readId && cy ? cy.getElementById(readId) : null;
    return {
      ok: true,
      readId,
      overlayGone: !document.querySelector('.rm-mask'),
      highlight: el && !el.empty() ? el.hasClass('search-hit') : null,
      zoom: cy ? Math.round(cy.zoom() * 100) / 100 : null,
      sidebarNode: document.querySelector('.sidebar h2')?.textContent?.trim() || null,
    };
  })()`);
  await sleep(400);
  await shot('read-ta-6-locate-in-graph');

  // 7.6) 再进阅读模式（按钮进入这一侧也要验），随后用 Esc 退出
  results.reenter = await ev(`(async () => {
    document.querySelector('.read-toggle').click();
    for (let i = 0; i < 200; i++) {
      if (document.querySelector('.rm-mask')) break;
      await new Promise(r => setTimeout(r, 10));
    }
    await new Promise(r => requestAnimationFrame(r));
    return {
      overlay: !!document.querySelector('.rm-mask'),
      treeTitle: document.querySelector('.rt-title')?.textContent?.replace(/\\s+/g, ' ').trim(),
      rows: document.querySelectorAll('.trow').length,
      readerId: document.querySelector('.rd-meta .chip.id')?.textContent?.trim(),
    };
  })()`);
  await sleep(300);

  // 8) 清筛选 → Esc 退出 → 图谱原状
  results.exit = await ev(`(async () => {
    const clear = document.querySelector('.rt-clear');
    if (clear) clear.click();
    await new Promise(r => setTimeout(r, 200));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await new Promise(r => setTimeout(r, 300));
    const cy = window.__engramDebug?.cy;
    return {
      overlayGone: !document.querySelector('.rm-mask'),
      buttonText: document.querySelector('.read-toggle')?.textContent?.trim(),
      graphNodes: cy ? cy.nodes().length : -1,
      graphVisible: !!document.querySelector('.cy-container')?.clientWidth,
      lsViewMode: localStorage.getItem('engram-view-mode'),
      sidebarNode: document.querySelector('.sidebar h2')?.textContent || null,
    };
  })()`);
  await sleep(400);
  await shot('read-ta-7-back-to-graph');

  // 9) 大图场景：perf1500（1500 节点，开发模式）——用户痛点原场景
  results.perf1500 = await ev(`(async () => {
    // 切到开发页签
    const devTab = [...document.querySelectorAll('.mode-tab')].find(t => t.textContent.includes('开发'));
    devTab.click();
    await new Promise(r => setTimeout(r, 400));
    const rows = [...document.querySelectorAll('.ws-row')];
    const row = rows.find(r => (r.querySelector('.ws-name')?.textContent || '').trim() === 'perf1500');
    if (!row) return { ok: false, why: 'no perf1500', names: rows.map(r => r.querySelector('.ws-name')?.textContent) };
    const wantPath = row.getAttribute('title') || '';
    row.click();
    const tLoad = performance.now();
    for (let i = 0; i < 400; i++) {
      const s = window.__engramDebug?.snapshot;
      const dirShown = (document.querySelector('.dir')?.textContent || '').trim();
      if (s && s.nodes.length > 100 && dirShown === wantPath) break;
      await new Promise(r => setTimeout(r, 150));
    }
    const loadMs = Math.round(performance.now() - tLoad);
    const s = window.__engramDebug?.snapshot;
    const t0 = performance.now();
    document.querySelector('.read-toggle').click();
    for (let i = 0; i < 600; i++) {
      if (document.querySelector('.rm-mask')) break;
      await new Promise(r => setTimeout(r, 10));
    }
    await new Promise(r => requestAnimationFrame(r));
    const enterMs = Math.round((performance.now() - t0) * 10) / 10;
    return {
      ok: true,
      nodes: s?.nodes?.length ?? -1,
      loadMs,
      enterMs,
      visibleRows: document.querySelectorAll('.trow').length,
      count: document.querySelector('.rm-count')?.textContent?.trim(),
    };
  })()`);
  await sleep(500);
  await shot('read-perf1500-1-enter');

  results.perf1500Expand = await ev(`(async () => {
    const t0 = performance.now();
    document.querySelector('.rt-tool')?.click();
    await new Promise(r => requestAnimationFrame(r));
    await new Promise(r => requestAnimationFrame(r));
    const expandMs = Math.round((performance.now() - t0) * 10) / 10;
    const list = document.querySelector('.rt-list');
    // 滚动压力：滚到底部
    const t1 = performance.now();
    list.scrollTop = list.scrollHeight;
    await new Promise(r => requestAnimationFrame(r));
    const scrollMs = Math.round((performance.now() - t1) * 10) / 10;
    return {
      expandMs,
      scrollMs,
      visibleRows: document.querySelectorAll('.trow').length,
      domNodes: document.querySelectorAll('.rm-mask *').length,
      scrollH: list.scrollHeight,
    };
  })()`);
  await sleep(400);
  await shot('read-perf1500-2-expand-all');

  results.perf1500Search = await ev(`(async () => {
    const input = document.querySelector('.rt-search');
    input.value = '渲染';
    const t0 = performance.now();
    input.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise(r => setTimeout(r, 400));
    const hits = document.querySelectorAll('.trow.match').length;
    return { searchMs: Math.round((performance.now() - t0) * 10) / 10, hits };
  })()`);
  await sleep(300);

  // 退出前复位（避免把测试视图留给用户）
  await ev(`(async () => {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await new Promise(r => setTimeout(r, 200));
    const q = document.querySelector('.rt-search');
    if (q) { q.value = ''; q.dispatchEvent(new Event('input', { bubbles: true })); }
    const devTab = [...document.querySelectorAll('.mode-tab')].find(t => t.textContent.includes('分析'));
    if (devTab) devTab.click();
    await new Promise(r => setTimeout(r, 600));
    return true;
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
