const D = window.__engramDebug;
const cy = D.cy;
await D.settle();
await new Promise((r) => setTimeout(r, 1600));

const check = () => {
  const boxes = [];
  cy.nodes().forEach((n) => {
    if (n.hasClass('no-label') || n.style('display') === 'none') return;
    const bb = n.boundingBox({ includeNodes: true, includeEdges: false, includeLabels: true, includeOverlays: false });
    boxes.push({
      id: n.id().slice(0, 10),
      x: bb.x1 * cy.zoom() + cy.pan().x,
      y: bb.y1 * cy.zoom() + cy.pan().y,
      w: bb.w * cy.zoom(),
      h: bb.h * cy.zoom(),
    });
  });
  let pairs = 0;
  let maxW = 0;
  const sample = [];
  for (let i = 0; i < boxes.length; i++) {
    maxW = Math.max(maxW, boxes[i].w);
    for (let j = i + 1; j < boxes.length; j++) {
      const a = boxes[i], b = boxes[j];
      if (a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y) {
        pairs++;
        if (sample.length < 5) sample.push(a.id + '×' + b.id);
      }
    }
  }
  return { 带标签: boxes.length, 重叠对: pairs, 最大盒宽: Math.round(maxW), 样例: sample };
};

const out = [];
out.push('全图: ' + JSON.stringify(check()));
const nd = cy.nodes().filter((n) => n.degree() >= 8 && n.style('display') !== 'none')[0];
nd.emit('dbltap');
await new Promise((r) => setTimeout(r, 1100));
await D.settle();
await new Promise((r) => setTimeout(r, 900));
out.push('聚焦[' + nd.id().slice(0, 8) + ']: ' + JSON.stringify(check()) + ' 点亮=' + D.focus.lit + ' 压暗=' + D.focus.dim);
return out.join('\n');
