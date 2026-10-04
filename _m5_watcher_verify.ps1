# _m5_watcher_verify.ps1 - M5 watcher acceptance:
# write one node via engram-mcp (stdio) into the user's dev workspace,
# then the user visually confirms the running GUI auto-refreshes within 2 seconds.
# ASCII only (PS 5.1 GBK parsing).

$ErrorActionPreference = "Stop"
$exe = "G:\test1.x\src-tauri\target\debug\engram-mcp.exe"
$cfgPath = Join-Path $env:APPDATA "com.chaingui.desktop\workspaces.json"

if (-not (Test-Path $cfgPath)) { throw "workspaces.json not found: $cfgPath" }
$list = Get-Content $cfgPath -Raw | ConvertFrom-Json
Write-Host "registered workspaces:"
$list | ForEach-Object { Write-Host ("  [" + $_.mode + "] " + $_.path + "  (" + $_.name + ")") }

$dev = @($list | Where-Object { $_.mode -eq "dev" -and $_.path -eq "G:\water" })
if ($dev.Count -eq 0) { throw "workspace G:\water not registered as dev" }
$ws = $dev[0].path
Write-Host ""
Write-Host ("target workspace: " + $ws)

$proc = $null
function Read-Line($timeoutMs = 8000) {
    $task = $proc.StandardOutput.ReadLineAsync()
    if (-not $task.Wait($timeoutMs)) { throw "TIMEOUT waiting response line" }
    return $task.Result
}

$script:reqId = 0
function Send-Req($method, $paramsJson) {
    $script:reqId++
    $json = '{"jsonrpc":"2.0","id":' + $script:reqId + ',"method":"' + $method + '","params":' + $paramsJson + '}'
    $proc.StandardInput.WriteLine($json)
    $proc.StandardInput.Flush()
    return Read-Line 8000
}

try {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.Arguments = '--workspace "' + $ws + '"'
    $psi.RedirectStandardInput  = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError  = $true
    $psi.UseShellExecute = $false
    $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $proc = [System.Diagnostics.Process]::Start($psi)

    $r = Send-Req "initialize" '{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"m5-verify","version":"0.1"}}'
    if (($r -replace '\\"', '"') -notmatch '"name"\s*:\s*"engram-mcp"') { throw "handshake failed: $r" }

    $stamp = Get-Date -Format "HHmmss"
    $title = "MCP Watcher Verify $stamp"
    $body  = "Written by engram-mcp at $stamp. If this node shows up in the running GUI within 2 seconds, watcher acceptance PASSES."
    $args  = '{"title":"' + $title + '","body":"' + $body + '","tags":["mcp-verify"]}'
    $r = Send-Req "tools/call" ('{"name":"create_node","arguments":' + $args + '}')
    $norm = $r -replace '\\"', '"'
    if ($norm -notmatch '"created"\s*:\s*true') { throw "create_node failed: $r" }
    $id = [regex]::Match($norm, '"id"\s*:\s*"([^"]+)"').Groups[1].Value

    Write-Host ""
    Write-Host ("NODE WRITTEN: id=" + $id + "  title=" + $title)
    Write-Host ("file: " + $ws + "\.chain\nodes\" + $id + ".md")
    Write-Host ">>> Check the running GUI NOW: the node should appear within 2 seconds. <<<"
}
finally {
    if ($proc) {
        try { $proc.StandardInput.Close() } catch {}
        try { if (-not $proc.WaitForExit(2000)) { $proc.Kill() } } catch {}
        $proc.Dispose()
    }
}
