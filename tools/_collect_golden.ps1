# _collect_golden.ps1 - 固化 engram-mcp 工具请求/响应对为 golden 契约文件（契约 v8）
# 与 crates/engram-mcp/tests/golden_contract.rs 的 replay_flow 完全一致（顺序即契约）。
# 契约 v8：11 工具（create/update/link/unlink/archive 已移除；写路径唯一入口 remember；
# 冻结自愈 resolve_conflict）。
# 注：本环境 PS 5.1 偶发 stdin 超时，UTF-8 干净的 Node 等价实现见 tools/_collect_golden.mjs。
# 输出：docs/test-golden/engram-mcp-golden.json（实现 MCP golden 契约测试时直接对照）
# 路径参数化：本地/CI 可用 -Exe/-Out 覆盖；默认从本脚本所在仓库根推导
param(
  [string]$Exe = (Join-Path (Split-Path $PSScriptRoot -Parent) "target\release\engram-mcp.exe"),
  [string]$Out = (Join-Path (Split-Path $PSScriptRoot -Parent) "docs\test-golden")
)
$ErrorActionPreference = "Stop"
$exe = $Exe
$out = $Out
New-Item -ItemType Directory -Force $out | Out-Null
$ws = Join-Path $env:TEMP ("engram_golden_" + $PID)
New-Item -ItemType Directory -Force (Join-Path $ws ".chain\nodes") | Out-Null
Set-Content (Join-Path $ws ".chain\.mode") "dev" -NoNewline
$golden = @()
$script:reqId = 0

$si = New-Object System.Diagnostics.ProcessStartInfo
$si.FileName = $exe
$si.Arguments = '--workspace "' + $ws + '"'
$si.RedirectStandardInput = $true; $si.RedirectStandardOutput = $true; $si.RedirectStandardError = $true
$si.UseShellExecute = $false
$p = [System.Diagnostics.Process]::Start($si)

# PS 5.1 陷阱：Process 重定向的 stdin/stdout 走 ANSI 编码（GBK），中文请求会被写坏、响应被读坏
# （传输静默死亡/乱码）——统一走 BaseStream 字节级 I/O + 显式 UTF-8，不依赖控制台代码页
$utf8 = New-Object System.Text.UTF8Encoding($false)
$inStream = $p.StandardInput.BaseStream
$outStream = $p.StandardOutput.BaseStream

function Read-Line($timeoutMs = 10000) {
  $mem = New-Object System.IO.MemoryStream
  $buf = New-Object byte[] 8192
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  while ($sw.ElapsedMilliseconds -lt $timeoutMs) {
    $task = $outStream.ReadAsync($buf, 0, $buf.Length)
    if (-not $task.Wait([math]::Max(1, [int]($timeoutMs - $sw.ElapsedMilliseconds)))) { throw "TIMEOUT" }
    $n = $task.Result
    if ($n -eq 0) { throw "EOF" }
    for ($i = 0; $i -lt $n; $i++) {
      $b = $buf[$i]
      if ($b -eq 10) { return $utf8.GetString($mem.ToArray()) }  # LF 行尾
      if ($b -ne 13) { $mem.WriteByte($b) }                       # 跳过 CR
    }
  }
  throw "TIMEOUT"
}
function Send($method, $paramsJson) {
  $script:reqId++
  $json = '{"jsonrpc":"2.0","id":' + $script:reqId + ',"method":"' + $method + '","params":' + $paramsJson + '}'
  $bytes = $utf8.GetBytes($json + "`n")
  $inStream.Write($bytes, 0, $bytes.Length); $inStream.Flush()
  return Read-Line
}
function Tool($name, $argsJson) {
  $req = '{"name":"' + $name + '","arguments":' + $argsJson + '}'
  $resp = Send "tools/call" $req
  $script:golden += @{ tool = $name; request = $req; response = $resp }
  return $resp
}

Send "initialize" '{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"golden","version":"1.0"}}' | Out-Null
Send "notifications/initialized" "{}" | Out-Null

Tool "remember" '{"session":"s-golden","kind":"msg","role":"user","text":"开始搭 golden 工作区"}' | Out-Null
# 防时序抖动：updated 为秒级精度，两节点跨秒创建保证 search 的 updated 倒序结果确定
# （Rust 侧 golden_contract.rs 有相同间隔；两端必须保持一致）
Start-Sleep -Milliseconds 1100
Tool "remember" '{"session":"s-golden","kind":"decision","decided":"keep","covers":[1,1],"reason":"建初始节点","commits":[{"op":"create","title":"Golden A","body":"# A\nnode A body"}]}' | Out-Null
Start-Sleep -Milliseconds 1100
Tool "remember" '{"session":"s-golden","kind":"decision","decided":"keep","reason":"建第二个节点","commits":[{"op":"create","title":"Golden B","body":"# B\nnode B body"}]}' | Out-Null
Tool "remember" '{"session":"s-golden","kind":"decision","decided":"keep","reason":"建立 solves 链","commits":[{"op":"link","from":"node-1","to":"node-2","rel":"solves"}]}' | Out-Null
Tool "get_overview" '{}' | Out-Null
Tool "search" '{"query":"Golden"}' | Out-Null
Tool "read_node" '{"id":"node-1","include_neighbors":true}' | Out-Null
Tool "expand" '{"id":"node-1","depth":2}' | Out-Null
Tool "read_path" '{"from":"node-1","to":"node-2"}' | Out-Null
Tool "get_guide" '{}' | Out-Null
Tool "remember" '{"session":"s-golden","kind":"decision","decided":"revise","reason":"补充结论","commits":[{"op":"update","id":"node-1","mode":"append","content":"\nappended note"}]}' | Out-Null
# 结构违规（词表外 rel）→ remember 阻断并报已执行意图数（决策行仍留痕）
Tool "remember" '{"session":"s-golden","kind":"decision","decided":"keep","reason":"词表外 rel 验证","commits":[{"op":"link","from":"node-1","to":"node-2","rel":"bogus"}]}' | Out-Null
# recall：golden 工作区无索引 → 关键词降级（mode=keyword,degraded=true），确定性无模型依赖
Tool "recall" '{"query":"Golden"}' | Out-Null
Tool "consolidate" '{}' | Out-Null
# 防时序抖动：骨架节点须跨秒创建，保证后续 recall 的 updated 倒序结果确定
# （Rust 侧 golden_contract.rs 有相同间隔；两端必须保持一致）
Start-Sleep -Milliseconds 1100
Tool "consolidate" '{"dry_run":false}' | Out-Null
Tool "remember" '{"session":"s-golden","kind":"decision","decided":"keep","reason":"拆链重排","commits":[{"op":"unlink","from":"node-1","to":"node-2"}]}' | Out-Null
Tool "remember" '{"session":"s-golden","kind":"decision","decided":"keep","reason":"内容过时","commits":[{"op":"archive","id":"node-2"}]}' | Out-Null
Tool "recall" '{"query":"Golden"}' | Out-Null
Tool "recall" '{"query":"Golden","include_archived":true}' | Out-Null
# ── 三层重构：账本状态 / 溯源 / 决策留痕 ──
Tool "dialogue_status" '{}' | Out-Null
Tool "remember" '{"session":"s-golden","kind":"msg","role":"user","text":"把这段整理成节点"}' | Out-Null
Tool "remember" '{"session":"s-golden","kind":"decision","decided":"keep","covers":[9,9],"nodes":["node-2"],"reason":"整理成节点","commits":[{"op":"create","title":"Golden C","body":"> 触发：Golden C\n\nfrom dialogue"}]}' | Out-Null
Tool "dialogue_status" '{}' | Out-Null
Tool "read_node" '{"id":"node-2"}' | Out-Null
# ── 三层重构 P2 契约 v8：并发冲突 → 冻结 → resolve_conflict 自愈 ──
Tool "remember" '{"session":"s-golden","kind":"decision","decided":"revise","reason":"并发冲突验证","commits":[{"op":"update","id":"node-1","mode":"replace_body","content":"冲突内容","expected_updated":"2000-01-01T00:00:00+08:00"}]}' | Out-Null
Tool "read_node" '{"id":"node-1"}' | Out-Null
Tool "resolve_conflict" '{"id":"node-1","title":"Golden A","status":"none","body":"# A\nnode A body（裁决后）"}' | Out-Null
Tool "read_node" '{"id":"node-1"}' | Out-Null

$p.Kill(); $p.WaitForExit()
$json = $golden | ConvertTo-Json -Depth 8; [System.IO.File]::WriteAllText((Join-Path $out "engram-mcp-golden.json"), $json, [System.Text.UTF8Encoding]::new($false))
Remove-Item $ws -Recurse -Force
"golden entries: $($golden.Count) -> docs/test-golden/engram-mcp-golden.json"
