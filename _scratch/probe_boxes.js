const D = window.__engramDebug;
const cy = D.cy;
await D.settle();
await new Promise((r) => setTimeout(r, 1800));

const z = cy.zoom();
const rows = [];
cy.nodes().forEach((n) => {
  if (n.hasClass('no-label') || n.style('display') === 'none') return;
  const nb = n.boundingBox({ includeNodes: true, includeEdges: false, includeLabels: false, includeOverlays: false });
  const lb = n.boundingBox({ includeNodes: false, includeEdges: false, includeLabels: true, includeOverlays: false });
  rows.push({
    id: n.id().slice(0, 10),
    fs世界: +parseFloat(n.style('font-size')).toFixed(1),
    换行宽世界: +parseFloat(n.style('text-max-width')).toFixed(1),
    节点世界: Math.round(nb.w) + 'x' + Math.round(nb.h),
    标签世界: Math.round(lb.w) + 'x' + Math.round(lb.h),
    标签屏幕: Math.round(lb.w * z) + 'x' + Math.round(lb.h * z),
  });
});
return JSON.stringify({ zoom: +z.toFixed(3), 带标签数: rows.length, 前8个: rows.slice(0, 8) }, null, 1);
