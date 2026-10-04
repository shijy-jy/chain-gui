const D = window.__engramDebug;
const cy = D.cy;
await D.settle();
await new Promise((r) => setTimeout(r, 1800));

const out = [];
out.push('URL=' + location.href);
out.push('节点=' + cy.nodes().length + '  布局=' + D.layout.info.mode + ' ' + D.layout.info.ms + 'ms');
out.push('图上名称自检=' + JSON.stringify(D.names));
out.push('工具栏=' + Array.from(document.querySelectorAll('.slider-label')).map((l) => l.textContent.trim().split(' ')[0]).join(' / '));

// 双击拉近 + 聚焦压暗
const nd = cy.nodes().filter((n) => n.degree() >= 3 && n.style('display') !== 'none')[0];
const z0 = cy.zoom();
nd.emit('dbltap');
await new Promise((r) => setTimeout(r, 1200));
await D.settle();
await new Promise((r) => setTimeout(r, 800));
const f = D.focus;
out.push('双击: zoom ' + z0.toFixed(3) + '→' + f.zoom.toFixed(3) + ' (' + (f.zoom / z0).toFixed(2) + '×) 居中=' + (Math.abs(nd.renderedPosition().x - cy.width() / 2) < 2) + ' 点亮/压暗=' + f.lit + '/' + f.dim);

// 聚焦态下点开节点看右侧信息栏
nd.emit('tap');
await new Promise((r) => setTimeout(r, 900));
const right = document.querySelector('.reader, .panel, .info-panel, aside:last-of-type, [class*=sb-]');
const txt = right ? String(right.textContent).replace(/\s+/g, ' ').trim().slice(0, 120) : '（未匹配到选择器）';
out.push('右侧信息栏片段=' + JSON.stringify(txt));
out.push('点开后图上名称自检=' + JSON.stringify(D.names));

// 静止性
const a = cy.nodes().map((n) => n.position('x').toFixed(3)).join('|');
await new Promise((r) => setTimeout(r, 1000));
const b = cy.nodes().map((n) => n.position('x').toFixed(3)).join('|');
out.push('静止性=' + (a === b ? '✓ 完全静止' : '✗ 仍在动'));

return out.join('\n');
