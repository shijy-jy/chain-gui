const cytoscape = require('cytoscape');
const { chainToElements } = require('./_cy_headless_lib.cjs');
const nodeSize = (ele) => 14 + Math.min(Math.sqrt(ele.degree()), 6) * 4;
const edgeBaseWidth = (ele) => {
  const s = Math.min(nodeSize(ele.source()), nodeSize(ele.target()));
  return 0.8 + (s - 14) * 0.05;
};
const style = [
  { selector: 'node', style: { shape: 'ellipse', label: 'data(label)', width: nodeSize, height: nodeSize, 'background-color': '#888888' } },
  { selector: 'node[nodeType = "note"]', style: { 'background-color': '#94a3b8' } },
  {
    selector: 'edge',
    style: {
      'width': edgeBaseWidth,
      'curve-style': 'bezier',
      'control-point-distances': '30px',
      'control-point-weights': 0.5,
      'line-cap': 'round',
      'line-color': 'rgba(148,163,184,0.45)',
      'line-fill': (ele) => (ele.cy().edges().length > 300 ? 'solid' : 'linear-gradient'),
      'line-gradient-stop-colors': 'data(gradColors)',
      'line-gradient-stop-positions': '0%, 100%',
      'target-arrow-shape': 'triangle',
      'target-arrow-color': 'data(tgtColor)',
      'arrow-scale': 0.55,
      'opacity': 0.5,
    },
  },
  { selector: 'edge:hover', style: { 'opacity': 1, 'width': (ele) => Math.min(edgeBaseWidth(ele) * 1.8, 3) } },
];
const snap = {
  nodes: [
    { id: 'g-001', type: 'goal', title: '根', parent: null, status: 'pending', created: '', updated: '', revision: 1, tags: [], evidence: [], body: '' },
    { id: 't-001', type: 'task', title: '任务', parent: 'g-001', status: 'pending', created: '', updated: '', revision: 1, tags: [], evidence: [], body: '' },
  ],
  edges: [ { parent: 'g-001', child: 't-001' } ],
  manifest: { root: 'g-001' },
};
try {
  const cy = cytoscape({ headless: true, styleEnabled: true, style, elements: chainToElements(snap) });
  const e = cy.getElementById('g-001->t-001');
  console.log('cy created OK. edge rendered style line-fill =', e.style('line-fill'), '| width =', e.style('width'));
  const node = cy.getElementById('g-001');
  console.log('node label =', node.style('label'), '| bg =', node.style('background-color'));
} catch (err) {
  console.error('CY INIT THREW:', err && err.message ? err.message : String(err));
  process.exitCode = 1;
}
