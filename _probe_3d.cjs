// 3D 可行性探针（只读）：查 WebView2 里 WebGL2 是否可用 + 当前 WebView/GPU + 现有图规模
// 用法：node _probe_3d.cjs
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

const EVAL = `(() => {
  const out = { url: location.href, ua: navigator.userAgent };
  const mk = () => { try { return document.createElement('canvas'); } catch (e) { return null; } };
  const c = mk();
  if (c) {
    const gl2 = c.getContext('webgl2');
    if (gl2) {
      out.webgl2 = true;
      out.maxTexture = gl2.getParameter(gl2.MAX_TEXTURE_SIZE);
      out.maxSamples = gl2.getParameter(gl2.MAX_SAMPLES);
      const dbg = gl2.getExtension('WEBGL_debug_renderer_info');
      out.renderer = dbg ? gl2.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : '(masked)';
      out.vendor = dbg ? gl2.getParameter(dbg.UNMASKED_VENDOR_WEBGL) : '(masked)';
      const inst = gl2.getExtension('ANGLE_instanced_arrays');
      out.instancedBuiltin = true;                 // WebGL2 内建 drawArraysInstanced
      out.vao = typeof gl2.createVertexArray === 'function';
    } else {
      out.webgl2 = false;
      const gl1 = c.getContext('webgl');
      out.webgl1 = !!gl1;
      if (gl1) {
        const dbg = gl1.getExtension('WEBGL_debug_renderer_info');
        out.renderer = dbg ? gl1.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : '(masked)';
      }
    }
  }
  out.devicePixelRatio = window.devicePixelRatio;
  out.deg = window.__engramDebug || null;
  if (out.deg && typeof out.deg.snapshot === 'function') {
    try {
      const s = out.deg.snapshot();
      out.snapshot = { nodes: s.nodes.length, edges: s.edges.length, mode: s.manifest ? s.manifest.mode : null };
    } catch (e) { out.snapshotErr = String(e); }
  }
  if (out.deg && out.deg.cy) {
    try {
      const cy = out.deg.cy;
      out.cy = { nodes: cy.nodes().length, edges: cy.edges().length, zoom: cy.zoom(), pan: cy.pan() };
    } catch (e) { out.cyErr = String(e); }
  }
  return out;
})()`;

(async () => {
  let targets;
  try {
    targets = await getJson(`http://127.0.0.1:${PORT}/json/list`);
  } catch (e) {
    console.log(`CDP 不可用（127.0.0.1:${PORT}）：${e.message}`);
    console.log('→ 说明：当前没有开了调试端口的 Engram 实例在跑；下面的静态分析不依赖这项。');
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
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws 连接失败')); });

  try {
    const gi = await cdp(ws, 'SystemInfo.getInfo').catch((e) => ({ err: e.message }));
    if (gi && gi.gpu && gi.gpu.auxAttributes) {
      console.log('=== WebView2 GPU ===');
      console.log('  device:  ', JSON.stringify(gi.gpu.devices && gi.gpu.devices[0] && gi.gpu.devices[0].deviceString));
      console.log('  driver:  ', gi.gpu.auxAttributes.glRenderer || '(n/a)');
      console.log('  feature: ', gi.gpu.featureStatus || '(n/a)');
    } else if (gi && gi.err) {
      console.log('SystemInfo.getInfo 不可用：', gi.err);
    }
  } catch (e) { console.log('SystemInfo 查询失败：', e.message); }

  const r = await cdp(ws, 'Runtime.evaluate', { expression: EVAL, returnByValue: true, awaitPromise: false });
  if (r.exceptionDetails) {
    console.log('页面求值异常：', JSON.stringify(r.exceptionDetails.exception || r.exceptionDetails));
  } else {
    console.log('=== 页面能力与当前图 ===');
    console.log(JSON.stringify(r.result.value, null, 2));
  }
  ws.close();
  process.exit(0);
})().catch((e) => { console.error('FATAL', e.message); process.exit(1); });
