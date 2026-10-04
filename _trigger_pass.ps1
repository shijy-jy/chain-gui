# 批量补触发句（参数化：工作区 + 节点触发句表）：read_node → 正文开头插入「> 触发：…」→ update_node replace_body（乐观锁）
param([string]$Workspace = "G:\deepseek\RESTRI")
$ErrorActionPreference = "Stop"
$mcp = "G:\test1.x\target\release\engram-mcp.exe"
$ws = $Workspace
$utf8 = New-Object System.Text.UTF8Encoding($false)

$si = New-Object System.Diagnostics.ProcessStartInfo
$si.FileName = $mcp
$si.Arguments = '--workspace "' + $ws + '"'
$si.RedirectStandardInput = $true; $si.RedirectStandardOutput = $true; $si.RedirectStandardError = $true
$si.UseShellExecute = $false
$p = [System.Diagnostics.Process]::Start($si)
$in = $p.StandardInput.BaseStream; $out = $p.StandardOutput.BaseStream
$id = 0
function Send($m, $j) {
  $script:id++
  $b = $utf8.GetBytes('{"jsonrpc":"2.0","id":' + $script:id + ',"method":"' + $m + '","params":' + $j + '}' + "`n")
  $in.Write($b, 0, $b.Length); $in.Flush()
  $mem = New-Object System.IO.MemoryStream; $buf = New-Object byte[] 8192
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  while ($sw.ElapsedMilliseconds -lt 30000) {
    $task = $out.ReadAsync($buf, 0, $buf.Length)
    if (-not $task.Wait(1000)) { continue }
    $n = $task.Result
    if ($n -eq 0) { break }
    for ($i = 0; $i -lt $n; $i++) {
      if ($buf[$i] -eq 10) { return $utf8.GetString($mem.ToArray()) }
      if ($buf[$i] -ne 13) { $mem.WriteByte($buf[$i]) }
    }
  }
  throw "EOF/TIMEOUT"
}
function Call($name, $params) { (Send "tools/call" ('{"name":"' + $name + '","arguments":' + $params + '}')) }
function JsonText($resp) { $j = $resp | ConvertFrom-Json; if ($j.error) { throw "MCP ERROR: " + $j.error.message }; return $j.result.content[0].text }

Send "initialize" '{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"trigger-pass","version":"1"}}' | Out-Null
Send "notifications/initialized" '{}' | Out-Null

$triggers = @{
  "g-001" = "ReSTIR DI GI 完全学会总目标；时空复用重采样学习；路径追踪学习"
  "d-001" = "ReSTIR 学习路线里程碑 M0-M5；restir_learn 渲染器设计；学习路径"
  "t-001" = "蒙特卡洛积分学习；重要性采样 MIS 推导；balance heuristic 实验"
  "t-002" = "restir_learn 渲染器搭建；渐进 GPU 路径追踪骨架；Step 1 ground truth"
  "t-003" = "渲染器中文注释；nvcc 中文注释约束；代码即教材"
  "t-004" = "SIR WRS 重采样学习；reservoir 数据结构；RIS 无偏推导"
  "t-005" = "ReSTIR 数学推导教学文章；时空复用公式；M-cap 偏差成对结构"
  "t-006" = "从渲染一张图到实时路径追踪；总纲教学文章；M0-M3 融会贯通"
  "v-001" = "M0 教学文章渲染方程到 MIS；概念辩证卡；从渲染方程到 MIS 推导"
}

foreach ($nid in @("g-001","d-001","t-001","t-002","t-003","t-004","t-005","t-006","v-001")) {
  $r = Call "read_node" (@{ id = $nid } | ConvertTo-Json -Compress)
  $node = JsonText $r | ConvertFrom-Json
  $updated = $node.updated
  $body = $node.body
  if ($body -match "^>\s*触发") {
    "$nid : 已有触发句，跳过"
    continue
  }
  $newBody = "> 触发：" + $triggers[$nid] + "`n`n" + $body
  $args = @{ id = $nid; mode = "replace_body"; content = $newBody; expected_updated = $updated } | ConvertTo-Json -Compress
  $r = Call "update_node" $args
  try {
    $res = JsonText $r | ConvertFrom-Json
    "$nid : updated=$($res.updated) revision=$($res.revision) ✓"
  } catch {
    "$nid : FAILED -> $_"
  }
}

$p.Kill(); $p.WaitForExit()
"DONE"
