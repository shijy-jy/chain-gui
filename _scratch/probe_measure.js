const c = document.createElement('canvas').getContext('2d');
const t = '任务 · 深度优化调研并产出详细实施方案与预期';
const out = [];
for (const f of ['11px "Helvetica Neue", Helvetica, Arial, sans-serif', '11px sans-serif', '11px Arial']) {
  c.font = f;
  out.push(f.replace('11px ', '').split(',')[0] + ' -> ' + c.measureText(t).width.toFixed(1) + 'px');
}
const D = window.__engramDebug;
const cy = D && D.cy;
if (cy) {
  const n = cy.nodes().filter((x) => !x.hasClass('no-label') && x.style('display') !== 'none')[0];
  if (n) {
    const fs2 = parseFloat(n.style('font-size'));
    const tmw = parseFloat(n.style('text-max-width'));
    const bb = n.boundingBox({ includeNodes: true, includeLabels: true, includeOverlays: false, includeEdges: false });
    out.push('样例节点=' + n.id().slice(0, 10) + ' 字号(世界)=' + fs2.toFixed(1) + ' 换行宽(世界)=' + tmw.toFixed(1) + ' zoom=' + cy.zoom().toFixed(3));
    out.push('该节点标签=' + JSON.stringify(String(n.data('label'))));
    out.push('渲染盒 世界=' + bb.w.toFixed(1) + 'x' + bb.h.toFixed(1) + ' → 屏幕=' + (bb.w * cy.zoom()).toFixed(1) + 'x' + (bb.h * cy.zoom()).toFixed(1));
    // 按"世界字号 = 屏幕字号/zoom"反推：测得屏幕宽度应约等于 canvas 在屏幕字号下的测量值
    const screenFont = fs2 * cy.zoom();
    c.font = '11px sans-serif';
    const w11 = c.measureText(String(n.data('label'))).width;
    out.push('按比例外推的屏幕文本宽=' + (w11 * (screenFont / 11)).toFixed(1) + 'px（换行上限=' + (tmw * cy.zoom()).toFixed(1) + 'px）');
  }
}
return out.join('\n');
