const cytoscape = require('cytoscape');
const cy = cytoscape({ headless: true, styleEnabled: true, elements: [], style: [
  { selector: 'node', style: { width: 20, height: 20, 'background-color': '#888' } },
  { selector: 'edge', style: { 'curve-style': 'bezier', 'line-fill': 'linear-gradient', 'target-arrow-shape': 'triangle' } },
] });
try {
  cy.add([
    { data: { id: 'a' } },
    { data: { id: 'b' } },
    {
      data: { id: 'a->b', source: 'a', target: 'b' },
      style: {
        'line-gradient-stop-colors': ['#a78bfa', '#22d3ee'],
        'line-gradient-stop-positions': ['0%', '100%'],
        'target-arrow-color': '#22d3ee',
        'width': 1.2,
      },
    },
  ]);
  const e = cy.getElementById('a->b');
  console.log('stops =', JSON.stringify(e.style('line-gradient-stop-colors')));
  console.log('positions =', JSON.stringify(e.style('line-gradient-stop-positions')));
  console.log('arrow color =', e.style('target-arrow-color'));
  console.log('PER-ELEMENT STYLE OK');
} catch (err) {
  console.error('THREW:', err.message);
  process.exitCode = 1;
}
