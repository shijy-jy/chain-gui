# 批量补触发句（通用）：读 triggers.json（{workspace: {nodeId: trigger}}）→ 逐工作区逐节点写入
$ErrorActionPreference = "Stop"
$mcp = "G:\test1.x\target\release\engram-mcp.exe"
$trig = Get-Content "G:\test1.x\_triggers.json" -Raw -Encoding UTF8 | ConvertFrom-Json
$utf8 = New-Object System.Text.UTF8Encoding($false)
$script:in = $null; $script:out = $null; $script:rid = 0; $script:proc = $null

function Open-Client($ws) {
  $si = New-Object System.Diagnostics.ProcessStartInfo
  $si.FileName = $mcp
  $si.Arguments = '--workspace "' + $ws + '"'
  $si.RedirectStandardInput = $true; $si.RedirectStandardOutput = $true; $si.RedirectStandardError = $true
  $si.UseShellExecute = $false
  $script:proc = [System.Diagnostics.Process]::Start($si)
  $script:in = $script:proc.StandardInput.BaseStream
  $script:out = $script:proc.StandardOutput.BaseStream
  $script:rid = 0
  Send-Raw "initialize" '{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"trigger-pass","version":"1"}}' | Out-Null
  Send-Raw "notifications/initialized" '{}' | Out-Null
}
function Send-Raw($m, $j) {
  $script:rid++
  $b = $utf8.GetBytes('{"jsonrpc":"2.0","id":' + $script:rid + ',"method":"' + $m + '","params":' + $j + '}' + "`n")
  $script:in.Write($b, 0, $b.Length); $script:in.Flush()
  $mem = New-Object System.IO.MemoryStream; $buf = New-Object byte[] 8192
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  while ($sw.ElapsedMilliseconds -lt 30000) {
    $task = $script:out.ReadAsync($buf, 0, $buf.Length)
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
function Call-Tool($name, $params) { (Send-Raw "tools/call" ('{"name":"' + $name + '","arguments":' + $params + '}')) }
function Json-Text($resp) { $j = $resp | ConvertFrom-Json; if ($j.error) { throw "MCP ERROR: " + $j.error.message }; return $j.result.content[0].text }

foreach ($wsProp in $trig.PSObject.Properties) {
  $ws = $wsProp.Name
  $table = $wsProp.Value
  "==== $ws"
  Open-Client $ws
  foreach ($nidProp in $table.PSObject.Properties) {
    $nid = $nidProp.Name
    $trigger = $nidProp.Value
    try {
      $r = Call-Tool "read_node" (@{ id = $nid } | ConvertTo-Json -Compress)
      $node = Json-Text $r | ConvertFrom-Json
      $updated = $node.updated
      $body = $node.body
      if ($body -match "^>\s*触发") {
        "$nid : 已有触发句，跳过"
        continue
      }
      $newBody = "> 触发：" + $trigger + "`n`n" + $body
      $args = @{ id = $nid; mode = "replace_body"; content = $newBody; expected_updated = $updated } | ConvertTo-Json -Compress
      $r = Call-Tool "update_node" $args
      $res = Json-Text $r | ConvertFrom-Json
      "$nid OK"
    } catch {
      "$nid : FAILED -> $_"
    }
  }
  $script:proc.Kill(); $script:proc.WaitForExit()
}
"DONE"
