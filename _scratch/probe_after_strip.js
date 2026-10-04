const D = window.__engramDebug;
const cy = D.cy;
await D.settle();
await new Promise((r) => setTimeout(r, 1800));

const out = [];
const texts = () => cy.nodes().filter((n) => (n.style('label') ?? '') !== '' || n.style('text-opacity') !== 0).length;
out.push('图上文字节点=' + texts() + '（应 0）');

// 布局仍正常
out.push('布局: mode=' + D.layout.info.mode + ' ms=' + D.layout.info.ms + ' 可见=' + D.visibleCount);

// 双击拉近仍正常
const nd = cy.nodes().filter((n) => n.degree() >= 3 && n.style('display') !== 'none')[0];
const z0 = cy.zoom();
nd.emit('dbltap');
await new Promise((r) => setTimeout(r, 1100));
await D.settle();
await new Promise((r) => setTimeout(r, 800));
const f = D.focus;
out.push('双击[' + nd.id().slice(0, 8) + ']: zoom ' + z0.toFixed(3) + '→' + f.zoom.toFixed(3) + '（' + (f.zoom / z0).toFixed(2) + '×）点亮=' + f.lit + ' 压暗=' + f.dim + ' 居中=' + (Math.abs(nd.renderedPosition().x - cy.width() / 2) < 2));
out.push('聚焦态图上文字节点=' + texts() + '（应 0）');

// 退出
nd.emit('dbltap');
await new Promise((r) => setTimeout(r, 1100));
await D.settle();
out.push('退出聚焦: zoom=' + cy.zoom().toFixed(3) + ' 点亮=' + D.focus.lit + ' 压暗=' + D.focus.dim);

// 静止性
const a = cy.nodes().map((n) => n.position('x').toFixed(3)).join('|');
await new Promise((r) => setTimeout(r, 1000));
const b = cy.nodes().map((n) => n.position('x').toFixed(3)).join('|');
out.push('静止性: ' + (a === b ? '✓ 完全静止' : '✗ 仍在动'));
return out.join('\n');
