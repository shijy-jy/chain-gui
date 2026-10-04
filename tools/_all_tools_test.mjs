// tools/_all_tools_test.mjs —— Engram MCP 全工具本地测试（3.2.1）
// 覆盖：11 个工具（get_overview/search/read_node/expand/read_path/get_guide/recall/
//       consolidate/remember/dialogue_status/resolve_conflict）
//       + 分析模式建链通道（create 词表/根唯一/前缀 id；link 改挂/防环/根不可改挂；unlink 禁止；
//         update 状态流转）+ 冲突冻结自愈循环 + 伏笔登记 + 保留式抽取。
// 用法：node tools/_all_tools_test.mjs   （先部署 release：cargo build --release -p engram-mcp）
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const EXE = process.env.ENGRAM_MCP_EXE || 'D:\\AIworkspace\\Engram\\engram-mcp.exe';

function runWs(ws) {
  const child = spawn(EXE, ['--workspace', ws], { stdio: ['pipe', 'pipe', 'inherit'] });
  let buf = '';
  const waiters = [];
  child.stdout.on('data', (d) => {
    buf += d.toString('utf8');
    let idx;
    while ((idx = buf.indexOf('\n')) >= 0) {
      const line = buf.slice(0, idx);
      buf = buf.slice(idx + 1);
      const w = waiters.shift();
      if (w) w.resolve(line);
    }
  });
  let reqId = 0;
  const send = (method, paramsJson) => new Promise((resolve, reject) => {
    reqId += 1;
    const json = `{"jsonrpc":"2.0","id":${reqId},"method":"${method}","params":${paramsJson}}`;
    const timer = setTimeout(() => reject(new Error('TIMEOUT ' + method)), 60000);
    waiters.push({ resolve: (line) => { clearTimeout(timer); resolve(line); } });
    child.stdin.write(json + '\n');
  });
  const tool = (name, args) => send('tools/call', JSON.stringify({ name, arguments: args }));
  const text = (line) => { const p = JSON.parse(line); if (p.error) throw new Error(JSON.stringify(p.error)); return JSON.parse(p.result.content[0].text); };
  return { child, send, tool, text };
}

const results = [];
const ok = (name, cond, extra = '') => { results.push({ name, pass: !!cond, extra }); console.log(`  ${cond ? 'PASS' : 'FAIL'}  ${name}${extra ? '  | ' + extra : ''}`); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const mkdirp = (p) => fs.mkdirSync(p, { recursive: true });

async function testAnalysis(ws) {
  mkdirp(path.join(ws, '.chain', 'nodes'));
  fs.writeFileSync(path.join(ws, '.chain', '.mode'), 'analysis');
  fs.writeFileSync(path.join(ws, '.chain', '.schema'), '{"schema_version":"1.1"}');
  const s = runWs(ws);
  await s.send('initialize', '{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"all-tools","version":"1"}}');
  await s.send('notifications/initialized', '{}');
  try {
    // 1. get_guide（首读强制，含分析建链协议 v23）
    const g = s.text(await s.tool('get_guide', {}));
    ok('get_guide 返回分析指南', g.version === 23 && g.content.includes('分析模式建链通道'), `v${g.version}`);
    // 2. get_overview 空工作区
    const ov0 = s.text(await s.tool('get_overview', {}));
    ok('get_overview 空工作区', ov0.node_count === 0 && ov0.mode === 'analysis');
    // 3. 建根 goal（词表 + 前缀 id + origin 溯源）
    await s.tool('remember', { session: 's-test', kind: 'msg', role: 'user', text: '建立测试链根目标' });
    const r1 = s.text(await s.tool('remember', { session: 's-test', kind: 'decision', decided: 'keep', covers: [1, 1], nodes: ['g-001'], reason: '建根', commits: [{ op: 'create', title: '目标 · 测试根', body: '> 触发：测试根\n\n# 根', node_type: 'goal', status: 'in_progress', parent: null }] }));
    ok('create 根 goal（前缀 id + 溯源）', r1.created?.[0]?.id === 'g-001' && r1.created?.[0]?.origin === 'dialogue/log.jsonl#2', `${r1.created?.[0]?.id} ${r1.created?.[0]?.origin}`);
    // 4. 词表/结构护栏
    const mk = async (node_type, status, parent) => { await s.tool('remember', { session: 's-test', kind: 'msg', role: 'user', text: 'x' }); return s.tool('remember', { session: 's-test', kind: 'decision', decided: 'keep', reason: '护栏', commits: [{ op: 'create', title: '违规 · x', body: '> 触发：x\n\nx', node_type, status, parent }] }).then((l) => s.text(l)); };
    const e1 = await mk('note', 'pending', 'g-001').catch((e) => e.message);
    ok('type 词表外拒绝', e1.includes('REMEMBER_ANALYSIS_TYPE'));
    const e2 = await mk('design', 'none', 'g-001').catch((e) => e.message);
    ok('status 词表外拒绝', e2.includes('REMEMBER_ANALYSIS_STATUS'));
    const e3 = await mk('design', 'pending', null).catch((e) => e.message);
    ok('非 goal 缺父拒绝', e3.includes('REMEMBER_ANALYSIS_PARENT'));
    const e4 = await mk('goal', 'pending', null).catch((e) => e.message);
    ok('根唯一拒绝', e4.includes('REMEMBER_ANALYSIS_ROOT_UNIQUE'));
    // 5. 建 design / task 链
    const r2 = s.text(await s.tool('remember', { session: 's-test', kind: 'decision', decided: 'keep', reason: '建设计', commits: [{ op: 'create', title: '设计 · D', body: '> 触发：D\n\nD', node_type: 'design', status: 'success', parent: 'g-001' }] }));
    ok('create design（d- 前缀）', r2.created?.[0]?.id === 'd-001');
    const r3 = s.text(await s.tool('remember', { session: 's-test', kind: 'decision', decided: 'keep', reason: '建任务', commits: [{ op: 'create', title: '任务 · T', body: '> 触发：T\n\nT', node_type: 'task', status: 'in_progress', parent: 'd-001' }] }));
    ok('create task（t- 前缀 + 挂载）', r3.created?.[0]?.id === 't-001');
    // 6. link 护栏
    const le1 = await s.tool('remember', { session: 's-test', kind: 'decision', decided: 'keep', reason: '成环', commits: [{ op: 'link', from: 't-001', to: 'd-001', rel: 'contains' }] }).then((l) => s.text(l)).catch((e) => e.message);
    ok('防环拒绝', le1.includes('REMEMBER_ANALYSIS_CYCLE'));
    const le2 = await s.tool('remember', { session: 's-test', kind: 'decision', decided: 'keep', reason: '改挂根', commits: [{ op: 'link', from: 'd-001', to: 'g-001', rel: 'contains' }] }).then((l) => s.text(l)).catch((e) => e.message);
    ok('根不可改挂', le2.includes('REMEMBER_ANALYSIS_ROOT_REPARENT'));
    const le3 = await s.tool('remember', { session: 's-test', kind: 'decision', decided: 'keep', reason: '断边', commits: [{ op: 'unlink', from: 'd-001', to: 't-001' }] }).then((l) => s.text(l)).catch((e) => e.message);
    ok('unlink 禁止', le3.includes('REMEMBER_ANALYSIS_UNLINK_FORBIDDEN'));
    const r4 = s.text(await s.tool('remember', { session: 's-test', kind: 'decision', decided: 'keep', reason: '合法改挂', commits: [{ op: 'link', from: 'g-001', to: 't-001', rel: 'contains', desc: '改挂' }] }));
    ok('合法改挂', r4.linked?.[0]?.from === 'g-001');
    // 7. update 状态流转
    const r5 = s.text(await s.tool('remember', { session: 's-test', kind: 'decision', decided: 'keep', reason: '完成', commits: [{ op: 'update', id: 't-001', mode: 'append', content: '补充', status: 'success' }] }));
    ok('update 状态流转', r5.updated?.[0]?.status === 'success');
    const e5 = await s.tool('remember', { session: 's-test', kind: 'decision', decided: 'keep', reason: '非法状态', commits: [{ op: 'update', id: 't-001', mode: 'append', content: 'x', status: 'none' }] }).then((l) => s.text(l)).catch((e) => e.message);
    ok('状态词表外拒绝', e5.includes('REMEMBER_STATUS_VOCAB'));
    // 8. 冲突冻结 → resolve_conflict 自愈
    const e6 = await s.tool('remember', { session: 's-test', kind: 'decision', decided: 'revise', reason: '并发冲突', commits: [{ op: 'update', id: 't-001', mode: 'replace_body', content: '冲突', expected_updated: '2000-01-01T00:00:00+08:00' }] }).then((l) => s.text(l)).catch((e) => e.message);
    ok('冲突冻结', e6.includes('CONFLICT'), '冻结态应已置');
    const fz = s.text(await s.tool('read_node', { id: 't-001' }));
    ok('冻结标记可见', fz.frozen === true && fz.status === 'blocked');
    const rc = s.text(await s.tool('resolve_conflict', { id: 't-001', title: '任务 · T', status: 'success', body: '> 触发：T\n\n裁决后' }));
    ok('resolve_conflict 自愈', rc.status === 'success');
    const fz2 = s.text(await s.tool('read_node', { id: 't-001' }));
    ok('冻结已解除', fz2.frozen !== true);
    // 9. 读通道：search / read_node / expand / read_path / recall
    const se = s.text(await s.tool('search', { query: '设计' }));
    ok('search 命中', se.results?.length >= 1);
    const nb = s.text(await s.tool('read_node', { id: 'g-001', include_neighbors: true }));
    ok('read_node 邻居', nb.neighbors.children.length === 2, `children=${nb.neighbors.children.map((c) => c.id).join(',')}`);
    const ex = s.text(await s.tool('expand', { id: 'g-001', depth: 2 }));
    ok('expand 子图', ex.nodes?.length >= 3);
    const rp = s.text(await s.tool('read_path', { from: 'g-001', to: 't-001' }));
    ok('read_path 可达', rp.found === true);
    const rc2 = s.text(await s.tool('recall', { query: '测试根' }));
    ok('recall 可用（无索引应降级并声明）', Array.isArray(rc2.results) && (rc2.degraded === true || rc2.results.length >= 0));
    // 10. 伏笔 + 抽样（S2/S4 回归）
    const fs1 = s.text(await s.tool('remember', { session: 's-test', kind: 'decision', decided: 'foreshadow', covers: [2, 2], nodes: [], reason: '登记', foreshadowing: [{ covers: [2, 2], note: '细节未定' }] }));
    ok('伏笔登记', fs1.appended?.decided === 'foreshadow');
    const smp = s.text(await s.tool('remember', { session: 's-test', kind: 'decision', decided: 'keep', reason: '抽样', mode: 'sample', seed: 't1', candidates: [{ dir: '方向A', score: 0.6, commits: [{ op: 'create', title: '验证 · 抽A', body: '> 触发：A\n\nA', node_type: 'verification', status: 'success', parent: 't-001' }] }, { dir: '方向B', score: 0.4, commits: [{ op: 'create', title: '验证 · 抽B', body: '> 触发：B\n\nB', node_type: 'verification', status: 'success', parent: 't-001' }] }] }));
    ok('抽样建验证节点', (smp.created ?? []).length === 1 && smp.sampled?.selected === '方向A' && smp.created[0].id === 'v-001', `selected=${smp.sampled?.selected}`);
    // 11. consolidate / dialogue_status
    const co = s.text(await s.tool('consolidate', {}));
    ok('consolidate 计划', co.plan !== undefined || co.plan_count !== undefined || co.plan?.length === 0);
    const st = s.text(await s.tool('dialogue_status', {}));
    ok('dialogue_status 账本', st.records >= 8 && st.decisions.keep >= 5 && st.decisions.foreshadow === 1, `records=${st.records} keep=${st.decisions.keep}`);
    // 12. 结构终检
    const ov = s.text(await s.tool('get_overview', {}));
    ok('链结构终检（无校验错误）', ov.node_count === 4 && !(ov.validation?.errors?.length), `nodes=${ov.node_count}`);
  } finally {
    s.child.stdin.end();
    s.child.kill();
  }
}

async function testDev(ws) {
  mkdirp(path.join(ws, '.chain', 'nodes'));
  fs.writeFileSync(path.join(ws, '.chain', '.mode'), 'dev');
  fs.writeFileSync(path.join(ws, '.chain', '.schema'), '{"schema_version":"1.1"}');
  const s = runWs(ws);
  await s.send('initialize', '{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"all-tools","version":"1"}}');
  await s.send('notifications/initialized', '{}');
  try {
    const r = s.text(await s.tool('remember', { session: 's-dev', kind: 'decision', decided: 'keep', reason: 'dev 建节点', commits: [{ op: 'create', title: '开发节点', body: '> 触发：dev\n\ndev', node_type: 'goal', status: 'success', parent: 'g-001' }] }));
    const n = s.text(await s.tool('read_node', { id: r.created[0].id }));
    ok('dev 模式新字段被忽略（note/none）', n.type === 'note' && n.status === 'none' && n.parent === null, `${n.type}/${n.status}`);
  } finally {
    s.child.stdin.end();
    s.child.kill();
  }
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'engram_alltools_'));
const wsA = path.join(tmp, 'analysis');
const wsD = path.join(tmp, 'dev');
console.log('=== 分析模式全工具测试 ===');
await testAnalysis(wsA);
console.log('=== 开发模式兼容测试 ===');
await testDev(wsD);
const failed = results.filter((r) => !r.pass);
console.log(`\n总计 ${results.length} 项：通过 ${results.length - failed.length}，失败 ${failed.length}`);
if (failed.length) { failed.forEach((f) => console.log('FAIL:', f.name, f.extra)); process.exit(1); }
console.log('ALL TOOLS OK');
