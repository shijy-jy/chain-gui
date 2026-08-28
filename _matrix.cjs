const cytoscape = require('cytoscape');
const { chainToElements } = require('./_lib.cjs');
const nodeSize = (ele) => 14 + Math.min(Math.sqrt(ele.degree()), 6) * 4;
const edgeBaseWidth = (ele) => { const s = Math.min(nodeSize(ele.source()), nodeSize(ele.target())); return 0.8 + (s - 14) * 0.05; };
const snap = {
  nodes: [
    { id: 'g-001', type: 'goal', title: '根', parent: null, status: 'pending', created: '', updated: '', revision: 1, tags: [], evidence: [], body: '' },
    { id: 't-001', type: 'task', title: '任务', parent: 'g-001', status: 'pending', created: '', updated: '', revision: 1, tags: [], evidence: [], body: '' },
  ],
  edges: [ { parent: 'g-001', child: 't-001' } ],
  manifest: { root: 'g-001' },
};
const base = {
  'width': 1.2,
  'curve-style': 'bezier',
  'line-cap': 'round',
  'line-gradient-stop-colors': 'data(gradColors)',
  'line-gradient-stop-positions': '0%, 100%',
  'target-arrow-shape': 'triangle',
  'target-arrow-color': 'data(tgtColor)',
  'arrow-scale': 0.55,
  'opacity': 0.5,
};
function run(name, edgeStyle, extraSelectors) {
  const style = [
    { selector: 'node', style: { shape: 'ellipse', label: 'data(label)', width: nodeSize, height: nodeSize, 'background-color': '#888888' } },
    { selector: 'edge', style: edgeStyle },
    ...(extraSelectors || []),
  ];
  const errors = [];
  const cy = cytoscape({ headless: true, styleEnabled: true, style, elements: chainToElements(snap), 
    onError: (e) => errors.push(String(e && e.message || e)) });
  try {
    const e = cy.getElementById('g-001->t-001');
    const fill = e.style('line-fill');
    const w = e.style('width');
    console.log(`[${name}] OK: line-fill=${fill} width=${w} errors=${errors.length}`);
  } catch (err) {
    console.log(`[${name}] THREW: ${err.message}`);
  }
}
// A: 静态 line-fill + 静态 width
run('A 全静态', { ...base, 'line-fill': 'linear-gradient' });
// B: 静态 line-fill + 函数 width
run('B 函数width', { ...base, 'line-fill': 'linear-gradient', 'width': edgeBaseWidth });
// C: 函数 line-fill + 静态 width
run('C 函数line-fill', { ...base, 'line-fill': (ele) => (ele.cy().edges().length > 300 ? 'solid' : 'linear-gradient') });
// D: 函数 line-fill + 函数 width
run('D 双函数', { ...base, 'line-fill': (ele) => (ele.cy().edges().length > 300 ? 'solid' : 'linear-gradient'), 'width': edgeBaseWidth });
// E: 全静态 + :hover 选择器
run('E hover选择器', { ...base, 'line-fill': 'linear-gradient' }, [{ selector: 'edge:hover', style: { opacity: 1 } }]);
