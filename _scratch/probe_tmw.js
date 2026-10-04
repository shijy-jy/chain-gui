const D = window.__engramDebug;
const cy = D.cy;
await D.settle();
await new Promise((r) => setTimeout(r, 1500));

const labeled = () => cy.nodes().filter((n) => !n.hasClass('no-label') && n.style('display') !== 'none');
const labelW = (n) => {
  const lb = n.boundingBox({ includeNodes: false, includeEdges: false, includeLabels: true, includeOverlays: false });
  return Math.round(lb.w * cy.zoom());
};
const out = [];
const n = labeled()[0];
if (!n) return 'no labeled node';
out.push('初始: 屏幕标签宽=' + labelW(n) + ' 样式text-max-width=' + n.style('text-max-width'));

// 实验 1：selector().style() 后写
cy.style().selector('node').style({ 'text-max-width': 60 }).update();
out.push('实验1 selector后写 60 : 屏幕宽=' + labelW(n) + ' 读到=' + n.style('text-max-width'));

// 实验 2：style() 数组整体替换
cy.style([
  ...cy.style().json(),
  { selector: 'node', style: { 'text-max-width': 40 } },
]);
out.push('实验2 追加样式表 40 : 屏幕宽=' + labelW(n) + ' 读到=' + n.style('text-max-width'));

// 实验 3：直接改数据的 label（预截断思路）
const orig = String(n.data('label'));
n.data('label', '任务 · 深度优化调研…');
out.push('实验3 预截断文本 : 屏幕宽=' + labelW(n));
n.data('label', orig);
return out.join('\n');
