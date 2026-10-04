// _probe_nav.cjs - 在真实工作区上验证契约 v9 的导航字段（只读工具）
// 用法：node _probe_nav.cjs <workspace>
const { spawn } = require('node:child_process');
const path = require('node:path');

const ws = process.argv[2];
if (!ws) {
  console.error('用法：node _probe_nav.cjs <workspace>');
  process.exit(2);
}
const exe = path.join(__dirname, 'target', 'release', 'engram-mcp.exe');
const child = spawn(exe, ['--workspace', ws], { stdio: ['pipe', 'pipe', 'inherit'] });

let buf = '';
const waiters = [];
child.stdout.on('data', (d) => {
  buf += d.toString('utf8');
  let i;
  while ((i = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, i);
    buf = buf.slice(i + 1);
    const w = waiters.shift();
    if (w) w(line);
  }
});

let id = 0;
function send(method, params) {
  id += 1;
  const json = `{"jsonrpc":"2.0","id":${id},"method":"${method}","params":${params}}`;
  return new Promise((res, rej) => {
    const t = setTimeout(() => rej(new Error('TIMEOUT ' + method)), 60000);
    waiters.push((line) => {
      clearTimeout(t);
      res(line);
    });
    child.stdin.write(json + '\n');
  });
}
function text(resp) {
  const r = JSON.parse(resp);
  if (r.error) return { error: r.error };
  const c = r.result && r.result.content;
  const t = c && c[0] && c[0].text;
  try {
    return JSON.parse(t);
  } catch {
    return t;
  }
}
async function tool(name, args) {
  const t0 = Date.now();
  const resp = await send(
    'tools/call',
    JSON.stringify({ name, arguments: args }),
  );
  const ms = Date.now() - t0;
  const v = text(resp);
  return { v, ms, bytes: resp.length };
}

(async () => {
  await send('initialize', '{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"probe","version":"1"}}');
  await send('notifications/initialized', '{}');

  const ovr = await tool('get_overview', {});
  const ov = ovr.v;
  console.log(`=== get_overview (${ovr.ms}ms, ${ovr.bytes}B)`);
  console.log('node_count=', ov.node_count, 'edge_count=', ov.edge_count);
  console.log('structure=', JSON.stringify(ov.structure));
  console.log('hubs=', (ov.entry_hubs || []).map((h) => `${h.id}(sub${h.subtree_size},deg${h.degree})`).join(' '));
  const rootId = ov.structure && ov.structure.root_ids && ov.structure.root_ids[0];
  console.log('picked root:', rootId);

  if (rootId) {
    const r = await tool('expand', { id: rootId, depth: 2, direction: 'children' });
    const down = r.v;
    console.log(`\n=== expand(root, depth2, children) (${r.ms}ms, ${r.bytes}B)`);
    console.log('direction=', down.direction, 'node_count=', down.node_count, 'edges=', (down.edges || []).length);
    for (const n of (down.nodes || []).slice(0, 10)) {
      console.log(
        `  hop=${n.hop} depth=${n.depth} children=${n.children_count} ${n.id} [${n.type}] ${n.title} :: ${JSON.stringify(n.first_line)}`,
      );
    }
    const r2 = await tool('expand', { id: rootId, depth: 1, direction: 'parents' });
    console.log('parents of root:', r2.v.node_count, (r2.v.nodes || []).map((n) => n.id + '@hop' + n.hop).join(','));
  }

  const str = await tool('dialogue_status', {});
  const st = str.v;
  console.log(`\n=== dialogue_status (${str.ms}ms, ${str.bytes}B)`);
  console.log('records=', st.records, 'coverage=', JSON.stringify(st.coverage).slice(0, 300));
  console.log('gaps=', (st.gaps || []).length, 'open_loops=', (st.open_loops || []).length);

  const sr = await tool('search', { query: '节点' });
  const s = sr.v;
  console.log(`\n=== search (${sr.ms}ms, ${sr.bytes}B) total=${s.total}`);
  for (const r of (s.results || []).slice(0, 3)) {
    console.log(`  ${r.id} depth=${r.depth} parent=${r.parent} children=${r.children_count} origin=${r.origin}`);
  }
  child.stdin.end();
  child.kill();
})();
