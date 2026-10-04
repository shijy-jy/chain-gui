const D = window.__engramDebug;
const cy = D.cy;
await D.settle();
await new Promise((r) => setTimeout(r, 1800));

// 重叠严重度：不只数"对"，还算实际重叠面积占比（轻微描边相触 vs 大面积压字 是两回事）
const severity = () => {
  const rects = [];
  cy.nodes().forEach((n) => {
    if (n.hasClass('no-label') || n.style('display') === 'none') return;
    const lb = n.boundingBox({ includeNodes: false, includeEdges: false, includeLabels: true, includeOverlays: false });
    rects.push({
      x: lb.x1 * cy.zoom() + cy.pan().x,
      y: lb.y1 * cy.zoom() + cy.pan().y,
      w: lb.w * cy.zoom(),
      h: lb.h * cy.zoom(),
    });
  });
  let pairs = 0;
  let area = 0;
  let worst = 0;
  for (let i = 0; i < rects.length; i++) {
    for (let j = i + 1; j < rects.length; j++) {
      const a = rects[i], b = rects[j];
      const ox = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
      const oy = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
      if (ox > 0 && oy > 0) {
        pairs++;
        const ov = ox * oy;
        area += ov;
        const frac = ov / Math.min(a.w * a.h, b.w * b.h);
        if (frac > worst) worst = frac;
      }
    }
  }
  const labArea = rects.reduce((s, r) => s + r.w * r.h, 0);
  return {
    标签数: rects.length,
    接触对: pairs,
    重叠面积占标签总面积: labArea ? +(area / labArea * 100).toFixed(1) + '%' : '0%',
    最严重一对重叠占比: +(worst * 100).toFixed(0) + '%',
  };
};

const out = [];
out.push('全图: ' + JSON.stringify(severity()));
const nd = cy.nodes().filter((n) => n.degree() >= 8 && n.style('display') !== 'none')[0];
const z0 = cy.zoom();
nd.emit('dbltap');
await new Promise((r) => setTimeout(r, 1200));
await D.settle();
await new Promise((r) => setTimeout(r, 900));
const f = D.focus;
out.push(
  '双击节点 ' + nd.id().slice(0, 8) +
  ': zoom ' + z0.toFixed(3) + '→' + f.zoom.toFixed(3) + '（放大 ' + (f.zoom / z0).toFixed(2) + '×）' +
  ' 点亮=' + f.lit + ' 压暗=' + f.dim +
  ' 标签=' + D.labels.renderedLabels,
);
out.push('聚焦态: ' + JSON.stringify(severity()));
nd.emit('dbltap');
await new Promise((r) => setTimeout(r, 1200));
await D.settle();
out.push('退出聚焦: zoom=' + cy.zoom().toFixed(3) + ' 点亮=' + D.focus.lit + ' 压暗=' + D.focus.dim);
return out.join('\n');
