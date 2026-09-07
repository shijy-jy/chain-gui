# _collect_golden.ps1 - 固化 engram-mcp 9 工具的请求/响应对为 golden 契约文件
# 输出：docs/test-golden/engram-mcp-golden.json（实现 MCP golden 契约测试时直接对照）
$ErrorActionPreference = "Stop"
$exe = "D:\AIworkspace\Engram\engram-mcp.exe"
$out = "G:\test1.x\docs\test-golden"
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

function Read-Line($timeoutMs = 10000) {
  $task = $p.StandardOutput.ReadLineAsync()
  if (-not $task.Wait($timeoutMs)) { throw "TIMEOUT" }
  return $task.Result
}
function Send($method, $paramsJson) {
  $script:reqId++
  $json = '{"jsonrpc":"2.0","id":' + $script:reqId + ',"method":"' + $method + '","params":' + $paramsJson + '}'
  $p.StandardInput.WriteLine($json); $p.StandardInput.Flush()
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

Tool "create_node" '{"title":"Golden A","body":"# A\nnode A body"}' | Out-Null
Tool "create_node" '{"title":"Golden B","body":"# B\nnode B body"}' | Out-Null
Tool "link_nodes" '{"from":"node-1","to":"node-2","rel_type":"solves"}' | Out-Null
Tool "get_overview" '{}' | Out-Null
Tool "search" '{"query":"Golden"}' | Out-Null
Tool "read_node" '{"id":"node-1","include_neighbors":true}' | Out-Null
Tool "expand" '{"id":"node-1","depth":2}' | Out-Null
Tool "read_path" '{"from":"node-1","to":"node-2"}' | Out-Null
Tool "get_guide" '{}' | Out-Null
Tool "update_node" '{"id":"node-1","mode":"append","content":"\nappended note"}' | Out-Null
Tool "link_nodes" '{"from":"node-1","to":"node-2","rel_type":"bogus"}' | Out-Null

$p.Kill(); $p.WaitForExit()
$golden | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $out "engram-mcp-golden.json") -Encoding UTF8
Remove-Item $ws -Recurse -Force
"golden entries: $($golden.Count) -> docs/test-golden/engram-mcp-golden.json"
