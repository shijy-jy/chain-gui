const D = window.__engramDebug;
const cy = D.cy;
await D.settle();
await new Promise((r) => setTimeout(r, 1500));

const countText = () =>
  cy.nodes().filter((n) => String(n.style('label') ?? '') !== '' || String(n.style('text-opacity')) !== '0').length;

const out = [];
out.push('初始: 图上文字节点=' + countText() + '（应 0）');

// 点开节点：图上仍需 0 文字，名称在右侧信息栏
const nd = cy.nodes().filter((n) => n.degree() >= 3 && n.style('display') !== 'none')[0];
nd.emit('tap');
await new Promise((r) => setTimeout(r, 800));
out.push('点开后: 图上文字节点=' + countText() + '（应 0）');

// 找信息栏标题（右侧常驻面板）
const cands = Array.from(document.querySelectorAll('*')).filter((e) => {
  const c = String(e.className || '');
  return /panel|info|detail|node-title/i.test(c) && e.children.length === 0 && e.textContent.trim();
});
const titles = cands.slice(0, 6).map((e) => String(e.className).slice(0, 24) + '=' + e.textContent.trim().slice(0, 24));
out.push('信息栏候选元素: ' + (titles.length ? titles.join(' | ') : '未找到'));

// 悬停浮层仍显示 id · 类型
nd.emit('mouseover');
await new Promise((r) => setTimeout(r, 400));
const tip = document.querySelector('.hover-tip, [class*=hover]');
out.push('悬停提示: ' + (tip ? JSON.stringify(String(tip.textContent).trim().slice(0, 40)) : '无'));

return out.join('\n');
