// _verify_installed.cjs —— 验证**已安装**的 Engram（D:\AIworkspace\Engram）
//   ① 安装目录的 engram-mcp.exe 走真实 stdio 协议，返回指南版本与结构字段
//   ② 安装目录的 engram-cli.exe 版本行
// 用法：node _verify_installed.cjs
const { spawn, execFileSync } = require('node:child_process');
const path = require('node:path');

const DIR = 'D:\\AIworkspace\\Engram';
const ws = process.argv[2] || 'G:\\ta';

const exe = path.join(DIR, 'engram-mcp.exe');
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
  return new Promise((res, rej) => {
    const t = setTimeout(() => rej(new Error('TIMEOUT ' + method)), 30000);
    waiters.push((line) => { clearTimeout(t); res(line); });
    child.stdin.write(`{"jsonrpc":"2.0","id":${id},"method":"${method}","params":${params}}\n`);
  });
}
function text(resp) {
  const r = JSON.parse(resp);
  if (r.error) return { error: r.error };
  try { return JSON.parse(r.result.content[0].text); } catch { return r.result.content[0].text; }
}
(async () => {
  console.log('MCP 二进制：', exe);
  await send('initialize', '{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"v","version":"1"}}');
  await send('notifications/initialized', '{}');
  const tools = JSON.parse((await send('tools/list', '{}')));
  console.log('工具数：', tools.result.tools.length, '->', tools.result.tools.map((t) => t.name).join(','));

  const ov = text(await send('tools/call', JSON.stringify({ name: 'get_overview', arguments: {} })));
  console.log('guide_version：', ov.guide_version, '| 节点：', ov.node_count, '| root_ids：', JSON.stringify(ov.structure.root_ids));

  const g = text(await send('tools/call', JSON.stringify({ name: 'get_guide', arguments: {} })));
  console.log('get_guide 版本：', g.version, '| 指南首行：', String(g.content || g.guide || '').split('\n')[0].slice(0, 40));

  const s = text(await send('tools/call', JSON.stringify({ name: 'search', arguments: { query: '节点' } })));
  const r0 = (s.results || [])[0] || {};
  console.log('search 首条结构字段：', JSON.stringify({ id: r0.id, degree: r0.degree, depth: r0.depth, children_count: r0.children_count, origin: r0.origin }));

  const ex = text(await send('tools/call', JSON.stringify({ name: 'expand', arguments: { id: ov.structure.root_ids[0], depth: 1, direction: 'children' } })));
  console.log('expand.direction：', ex.direction, '| 节点数：', ex.node_count, '| 首节点首行：', JSON.stringify((ex.nodes[0] || {}).first_line));

  const st = text(await send('tools/call', JSON.stringify({ name: 'dialogue_status', arguments: {} })));
  console.log('coverage：', JSON.stringify(st.coverage && { total: st.coverage.total, read: st.coverage.read, unread: st.coverage.unread }));
  child.stdin.end();
  child.kill();

  try {
    const v = execFileSync(path.join(DIR, 'engram-cli.exe'), ['--version'], { encoding: 'utf8' });
    console.log('CLI 版本行：', v.trim());
  } catch (e) {
    console.log('CLI --version 失败：', e.message.slice(0, 120));
  }
})();
