const D = window.__engramDebug;
const cy = D.cy;
await D.settle();
await new Promise((r) => setTimeout(r, 1800));

const check = () => {
  const boxes = [];
  cy.nodes().forEach((n) => {
    if (n.hasClass('no-label') || n.style('display') === 'none') return;
    const nb = n.boundingBox({ includeNodes: true, includeEdges: false, includeLabels: false, includeOverlays: false });
    const lb = n.boundingBox({ includeNodes: false, includeEdges: false, includeLabels: true, includeOverlays: false });
    boxes.push({
      id: n.id().slice(0, 10),
      label: String(n.data('label')).slice(0, 18),
      x: nb.x1 * cy.zoom() + cy.pan().x,
      y: nb.y1 * cy.zoom() + cy.pan().y,
      w: Math.max(nb.w, lb.w) * cy.zoom(),
      h: (nb.h + lb.h) * cy.zoom(),
      labW: Math.round(lb.w * cy.zoom()),
    });
  });
  let pairs = 0;
  const samples = [];
  for (let i = 0; i < boxes.length; i++) {
    for (let j = i + 1; j < boxes.length; j++) {
      const a = boxes[i], b = boxes[j];
      if (a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y) {
        pairs++;
        if (samples.length < 3) samples.push(a.id + '×' + b.id);
      }
    }
  }
  const maxLab = boxes.reduce((m, b) => Math.max(m, b.labW), 0);
  return { 带标签: boxes.length, 重叠对: pairs, 最大标签屏宽: maxLab, 样例: samples, 首例: boxes[0] };
};

const out = [];
out.push('全图: ' + JSON.stringify(check()));
const nd = cy.nodes().filter((n) => n.degree() >= 8 && n.style('display') !== 'none')[0];
nd.emit('dbltap');
await new Promise((r) => setTimeout(r, 1200));
await D.settle();
await new Promise((r) => setTimeout(r, 1000));
out.push('聚焦: ' + JSON.stringify(check()));
// 再放大一档，确认截断随 zoom 自适应且不重叠
cy.zoom({ level: cy.zoom() * 1.8, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } });
D.relabel();
await new Promise((r) => setTimeout(r, 700));
out.push('再放大: ' + JSON.stringify(check()));
return out.join('\n');
