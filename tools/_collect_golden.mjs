// tools/_collect_golden.mjs - 固化 engram-mcp 工具请求/响应对为 golden 契约文件（契约 v9）
// 与 crates/engram-mcp/tests/golden_contract.rs 的 replay_flow 完全一致（顺序即契约）。
// PS 版 _collect_golden.ps1 在本环境偶发 stdin 超时，此为 UTF-8 干净的 Node 等价实现。
// 契约 v9：写路径唯一入口 remember；冻结自愈 resolve_conflict；结构导航增强
// （expand.direction / 逐节点 hop+first_line、dialogue_status.coverage）。
// 用法：node tools/_collect_golden.mjs
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const exe = path.join(root, 'target', 'release', 'engram-mcp.exe');
const out = path.join(root, 'docs', 'test-golden', 'engram-mcp-golden.json');
const ws = fs.mkdtempSync(path.join(os.tmpdir(), 'engram_golden_'));
fs.mkdirSync(path.join(ws, '.chain', 'nodes'), { recursive: true });
fs.writeFileSync(path.join(ws, '.chain', '.mode'), 'dev');

const child = spawn(exe, ['--workspace', ws], { stdio: ['pipe', 'pipe', 'inherit'] });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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
function send(method, paramsJson) {
  reqId += 1;
  const json = `{"jsonrpc":"2.0","id":${reqId},"method":"${method}","params":${paramsJson}}`;
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`TIMEOUT: ${method}`)), 15000);
    waiters.push({
      resolve: (line) => {
        clearTimeout(timer);
        resolve(line);
      },
    });
    child.stdin.write(json + '\n');
  });
}

const golden = [];
async function tool(name, argsJson) {
  const req = `{"name":"${name}","arguments":${argsJson}}`;
  const resp = await send('tools/call', req);
  golden.push({ tool: name, request: req, response: resp });
}

try {
  await send('initialize', '{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"golden","version":"1.0"}}');
  await send('notifications/initialized', '{}'); // rmcp 回 -32601 错误行，仅消费对齐

  await tool('remember', '{"session":"s-golden","kind":"msg","role":"user","text":"开始搭 golden 工作区"}');
  await sleep(1100); // updated 秒级精度：跨秒创建保证 search 倒序确定
  await tool('remember', '{"session":"s-golden","kind":"decision","decided":"keep","covers":[1,1],"reason":"建初始节点","commits":[{"op":"create","title":"Golden A","body":"# A\\nnode A body"}]}');
  await sleep(1100);
  await tool('remember', '{"session":"s-golden","kind":"decision","decided":"keep","reason":"建第二个节点","commits":[{"op":"create","title":"Golden B","body":"# B\\nnode B body"}]}');
  await tool('remember', '{"session":"s-golden","kind":"decision","decided":"keep","reason":"建立 solves 链","commits":[{"op":"link","from":"node-1","to":"node-2","rel":"solves"}]}');
  await tool('get_overview', '{}');
  await tool('search', '{"query":"Golden"}');
  await tool('read_node', '{"id":"node-1","include_neighbors":true}');
  await tool('expand', '{"id":"node-1","depth":2}');
  await tool('read_path', '{"from":"node-1","to":"node-2"}');
  await tool('get_guide', '{}');
  await tool('remember', '{"session":"s-golden","kind":"decision","decided":"revise","reason":"补充结论","commits":[{"op":"update","id":"node-1","mode":"append","content":"\\nappended note"}]}');
  // 结构违规（词表外 rel）→ remember 阻断并报已执行意图数（决策行仍留痕）
  await tool('remember', '{"session":"s-golden","kind":"decision","decided":"keep","reason":"词表外 rel 验证","commits":[{"op":"link","from":"node-1","to":"node-2","rel":"bogus"}]}');
  await tool('recall', '{"query":"Golden"}');
  await tool('consolidate', '{}');
  await sleep(1100); // 骨架节点跨秒创建，保证 recall 的 updated 倒序确定
  await tool('consolidate', '{"dry_run":false}');
  await tool('remember', '{"session":"s-golden","kind":"decision","decided":"keep","reason":"拆链重排","commits":[{"op":"unlink","from":"node-1","to":"node-2"}]}');
  await tool('remember', '{"session":"s-golden","kind":"decision","decided":"keep","reason":"内容过时","commits":[{"op":"archive","id":"node-2"}]}');
  await tool('recall', '{"query":"Golden"}');
  await tool('recall', '{"query":"Golden","include_archived":true}');
  await tool('dialogue_status', '{}');
  await tool('remember', '{"session":"s-golden","kind":"msg","role":"user","text":"把这段整理成节点"}');
  await tool('remember', '{"session":"s-golden","kind":"decision","decided":"keep","covers":[9,9],"nodes":["node-2"],"reason":"整理成节点","commits":[{"op":"create","title":"Golden C","body":"> 触发：Golden C\\n\\nfrom dialogue"}]}');
  await tool('dialogue_status', '{}');
  await tool('read_node', '{"id":"node-2"}');
  // ── 三层重构 P2 契约 v8：并发冲突 → 冻结 → resolve_conflict 自愈 ──
  await tool('remember', '{"session":"s-golden","kind":"decision","decided":"revise","reason":"并发冲突验证","commits":[{"op":"update","id":"node-1","mode":"replace_body","content":"冲突内容","expected_updated":"2000-01-01T00:00:00+08:00"}]}');
  await tool('read_node', '{"id":"node-1"}');
  await tool('resolve_conflict', '{"id":"node-1","title":"Golden A","status":"none","body":"# A\\nnode A body（裁决后）"}');
  await tool('read_node', '{"id":"node-1"}');
} finally {
  child.stdin.end();
  child.kill();
}

fs.writeFileSync(out, JSON.stringify(golden, null, 0) + '\n');
fs.rmSync(ws, { recursive: true, force: true });
console.log(`golden entries: ${golden.length} -> ${out}`);
