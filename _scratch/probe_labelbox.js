const D = window.__engramDebug;
const cy = D.cy;
await D.settle();
await new Promise((r) => setTimeout(r, 1500));

const samples = cy.nodes().slice(0, 5).map((n) => {
  const lb = n.boundingBox({ includeNodes: false, includeEdges: false, includeLabels: true, includeOverlays: false });
  return {
    id: n.id().slice(0, 8),
    label: JSON.stringify(String(n.style('label'))),
    textOpacity: JSON.stringify(String(n.style('text-opacity'))),
    标签盒: Math.round(lb.w) + 'x' + Math.round(lb.h),
  };
});

// 全量统计：标签盒面积 > 0 的节点数（>0 就意味着真的画了字）
let withTextBox = 0;
cy.nodes().forEach((n) => {
  const lb = n.boundingBox({ includeNodes: false, includeEdges: false, includeLabels: true, includeOverlays: false });
  if (lb.w > 0.01 || lb.h > 0.01) withTextBox++;
});

return JSON.stringify(
  {
    'D.names（自检）': D.names,
    标签盒非零的节点数: withTextBox,
    样本: samples,
  },
  null,
  1,
);
