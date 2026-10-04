// _verify_p2.cjs —— P2 四项的实跑验收（只读；CDP 驱动真实窗口）
//   ① 结构指标入接口：快照节点带 degree/depth/children_count/subtree_size（球径与 AI 同源）
//   ② 分析模式层级球壳布局生效（effectiveAlgo='hierarchy'），开发模式保持神经元
//   ③ 覆盖遮挡：图例默认折叠 + insets 量出来（相机取景避开浮层）
//   ④ 冷启动扫描（由目标工作区的加载耗时观察，配合 cargo 侧基准）
// 用法：node _verify_p2.cjs <workspacePathSubstring> <screenshot.png> [--click]
const http = require('http');
const fs = require('fs');
const path = require('path');
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
async function evalJs(ws, expr) {
  const r = await cdp(ws, 'Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  return r.result.value;
}

(async () => {
  const want = process.argv[2] || 'G:\\ta';
  const out = process.argv[3] || '_shots/p2.png';
  const doClick = process.argv.includes('--click');

  const targets = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  const page = targets.find((t) => t.type === 'page');
  if (!page) throw new Error('没有 page 目标');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  const errors = [];
  ws.onmessage = (m) => {
    const d = JSON.parse(m.data);
    if (d.id && pending.has(d.id)) {
      const p = pending.get(d.id);
      pending.delete(d.id);
      d.error ? p.reject(new Error(d.error.message)) : p.resolve(d.result);
      return;
    }
    if (d.method === 'Runtime.exceptionThrown') {
      errors.push('EXCEPTION: ' + (d.params.exceptionDetails.exception?.description || d.params.exceptionDetails.text || '').slice(0, 300));
    }
  };
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws 连接失败')); });
  await cdp(ws, 'Runtime.enable');
  await cdp(ws, 'Page.enable');

  // 强制整页重载：确保拿到最新前端模块（HMR 在嵌套组件上可能不生效，会验到旧代码）。
  // --noreload：刚重启过 app（天然加载最新模块）时跳过——Page.reload 会让 WebView2 画面变白
  // （本环境虚拟桌面合成器不出帧），渲染器侧截图会拍到空白。
  if (!process.argv.includes('--noreload')) {
    await cdp(ws, 'Page.reload', { ignoreCache: true });
    for (let i = 0; i < 40; i++) {
      await sleep(500);
      const ready = await evalJs(ws, `(() => !!(window.__engramDebug && document.querySelector('.ws-row')))()`).catch(() => false);
      if (ready) break;
    }
    console.log('页面已重载并挂载完成');
  } else {
    console.log('跳过重载（--noreload）');
  }

  // ── 先切模式页签（分析/开发），再点工作区行 ──
  const modeArg = (process.argv.find((a) => a.startsWith('--mode=')) || '').split('=')[1];
  if (modeArg === 'analysis' || modeArg === 'dev') {
    const ok = await evalJs(ws, `(() => {
      const tabs = Array.from(document.querySelectorAll('.mode-tab'));
      const want = ${JSON.stringify(modeArg === 'analysis' ? '分析' : '开发')};
      const t = tabs.find((x) => x.innerText.includes(want));
      if (!t) return false;
      t.click();
      return true;
    })()`);
    console.log(`→ 切模式页签 ${modeArg}：${ok ? 'ok' : '未找到'}`);
    await sleep(1200);
  }

  // ── 切到目标工作区（工作区栏点选；找不到就报告当前工作区）──
  const listed = await evalJs(ws, `(() => Array.from(document.querySelectorAll('.ws-row')).map((r) => r.innerText.replace(/\\s+/g, ' ').trim()))()`);
  console.log('工作区列表：', JSON.stringify(listed, null, 1));
  const idx = listed.findIndex((t) => t.toLowerCase().includes(want.toLowerCase()));
  if (idx >= 0) {
    await evalJs(ws, `(() => { const r = document.querySelectorAll('.ws-row')[${idx}]; r.click(); return true; })()`);
    console.log(`→ 已点选工作区 #${idx}：${listed[idx]}`);
  } else {
    console.log(`⚠ 列表里没有包含 "${want}" 的工作区；按当前工作区继续`);
  }
  // 等图加载（节点数 > 0 且稳定）
  let nodes = 0;
  const t0 = Date.now();
  for (let i = 0; i < 60; i++) {
    await sleep(500);
    nodes = await evalJs(ws, `(() => { const d = window.__engramDebug; return d && d.snapshot ? d.snapshot.nodes.length : 0; })()`);
    if (nodes > 0 && i >= 2) break;
  }
  console.log(`图加载完成：${nodes} 节点，等待 ${((Date.now() - t0) / 1000).toFixed(1)}s`);

  const state = await evalJs(ws, `(() => {
    const d = window.__engramDebug;
    const ws = document.querySelector('.legend');
    const zc = document.querySelector('.zoom-controls');
    const ns = document.querySelector('.node-search');
    const rect = (el) => { if (!el) return null; const r = el.getBoundingClientRect(); return { w: Math.round(r.width), h: Math.round(r.height), l: Math.round(r.left), t: Math.round(r.top) }; };
    return {
      mode: d.mode,
      layout: d.layout,
      nodes: d.snapshot.nodes.length,
      edges: d.snapshot.edges.length,
      // ① 结构注解抽样（取度最大的 3 个 + 前 2 个）
      annotated: d.snapshot.nodes.filter((n) => typeof n.degree === 'number').length,
      topByDegree: d.snapshot.nodes.slice().sort((a, b) => (b.degree ?? 0) - (a.degree ?? 0)).slice(0, 4)
        .map((n) => ({ id: n.id, degree: n.degree, depth: n.depth, children: n.children_count, subtree: n.subtree_size })),
      firstTwo: d.snapshot.nodes.slice(0, 2).map((n) => ({ id: n.id, degree: n.degree, depth: n.depth, children: n.children_count, subtree: n.subtree_size })),
      // ② 布局
      hierShells: (() => {
        // 用 __engramDebug 拿不到坐标：改为断言算法选择 + 壳半径公式的可观测前提（层距/深度直方图）
        const hist = {};
        for (const n of d.snapshot.nodes) hist[n.depth] = (hist[n.depth] ?? 0) + 1;
        return hist;
      })(),
      // ③ 覆盖遮挡
      legendFolded: !!document.querySelector('.legend.folded'),
      legendRect: rect(ws),
      zoomRect: rect(zc),
      searchRect: rect(ns),
      canvas: rect(document.querySelector('canvas')),
      insets: d.layout.insets,
      text: document.body.innerText.slice(0, 0),
    };
  })()`);
  console.log('=== 状态 ===');
  console.log(JSON.stringify(state, null, 1));

  // ── 可选：点画布找一个节点，验信息栏「结构」行 ──
  if (doClick) {
    // 信息栏可能处于收起态（细条）：先展开，否则节点详情不在 DOM 里
    await evalJs(ws, `(() => { const b = document.querySelector('.sidebar.collapsed .expand-btn'); if (b) { b.click(); return 'expanded'; } return 'already'; })()`);
    await sleep(500);
    const c = state.canvas;
    if (c) {
      const pts = [];
      // 细网格扫（节点球在屏幕上有直径，密集扫描能稳定命中一个）
      const NX = 17, NY = 13;
      for (let iy = 1; iy < NY; iy++) for (let ix = 1; ix < NX; ix++) pts.push([ix / NX, iy / NY]);
      let hit = null;
      let tried = 0;
      for (const [fx, fy] of pts) {
        const x = Math.round(c.l + c.w * fx), y = Math.round(c.t + c.h * fy);
        tried++;
        await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
        await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
        await cdp(ws, 'Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
        await sleep(45);
        if (tried % 20 === 0) {
          hit = await evalJs(ws, `(() => { const r = document.querySelector('.struct-row'); if (!r) return null; const h = document.querySelector('.id-row h2'); return { id: h ? h.innerText : null, struct: r.innerText.replace(/\\s+/g, ' ').trim() }; })()`);
          if (hit) break;
        }
      }
      if (!hit) {
        hit = await evalJs(ws, `(() => { const r = document.querySelector('.struct-row'); if (!r) return null; const h = document.querySelector('.id-row h2'); return { id: h ? h.innerText : null, struct: r.innerText.replace(/\\s+/g, ' ').trim() }; })()`);
      }
      console.log(`=== 信息栏结构行（扫了 ${tried} 个点）===`);
      console.log(JSON.stringify(hit));
    }
  }

  // ── 截图（渲染器侧）──
  const shot = await cdp(ws, 'Page.captureScreenshot', { format: 'png', fromSurface: false, captureBeyondViewport: false });
  fs.mkdirSync(path.dirname(path.resolve(out)), { recursive: true });
  fs.writeFileSync(out, Buffer.from(shot.data, 'base64'));
  console.log(`截图：${out} ${(fs.statSync(out).size / 1024).toFixed(1)} KB`);
  console.log('控制台异常：', errors.length ? JSON.stringify(errors, null, 1) : '无');
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
