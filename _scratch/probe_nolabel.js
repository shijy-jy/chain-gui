const D = window.__engramDebug;
const cy = D.cy;
await D.settle();
await new Promise((r) => setTimeout(r, 1800));

const out = [];
const visible = cy.nodes().filter((n) => n.style('display') !== 'none');
// 1) 数据里已无 label 字段
const withLabelData = visible.filter((n) => (n.data('label') ?? '') !== '').length;
// 2) 样式上文字不可见（text-opacity=0 或 label 为空）
const withVisibleText = visible.filter((n) => n.style('text-opacity') !== 0 || (n.style('label') ?? '') !== '').length;
// 3) 渲染包围盒应约等于纯圆点（没有文字撑宽）
const widths = visible.slice(0, 6).map((n) => Math.round(n.width()));
out.push('可见节点=' + visible.length);
out.push('数据里带 label 的节点=' + withLabelData + '（应为 0）');
out.push('样式上文字可见的节点=' + withVisibleText + '（应为 0）');
out.push('节点宽度（世界）=' + widths.join(', ') + '  ← 应等于纯圆点直径（无文字撑宽）');
out.push('调试钩子 names=' + JSON.stringify(D.names));

// 4) 点开节点：名称走侧栏，图上仍无文字
const nd = visible.filter((n) => n.degree() >= 3)[0];
nd.emit('tap');
await new Promise((r) => setTimeout(r, 600));
const side = document.querySelector('.panel-title, .rb-title, [class*=node-title]');
out.push('点击后 图上文字可见节点=' + visible.filter((n) => n.style('text-opacity') !== 0 || (n.style('label') ?? '') !== '').length + '（应仍为 0）');
out.push('点击后 侧栏标题元素=' + (side ? JSON.stringify(String(side.textContent).trim().slice(0, 30)) : '未找到（选择器需调整）'));

// 5) 悬停提示仍显示 id
nd.emit('mouseover');
await new Promise((r) => setTimeout(r, 400));
const tip = document.querySelector('[class*=hover]');
out.push('悬停提示=' + (tip ? JSON.stringify(String(tip.textContent).trim().slice(0, 40)) : '无'));
return out.join('\n');
