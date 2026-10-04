const D = window.__engramDebug;
const cy = D.cy;
await D.settle();
await new Promise((r) => setTimeout(r, 1200));
const n = cy.nodes().filter((x) => x.style('display') !== 'none')[0];
return JSON.stringify(
  {
    节点: n.id(),
    'style(label)': n.style('label'),
    'data(label)': n.data('label'),
    'style(text-opacity)': n.style('text-opacity'),
    'style(font-size)': n.style('font-size'),
    'style(text-wrap)': n.style('text-wrap'),
    'style(min-zoomed-font-size)': n.style('min-zoomed-font-size'),
    'style(text-outline-width)': n.style('text-outline-width'),
    标签盒: (() => {
      const lb = n.boundingBox({ includeNodes: false, includeLabels: true, includeOverlays: false, includeEdges: false });
      return Math.round(lb.w) + 'x' + Math.round(lb.h);
    })(),
  },
  null,
  1,
);
