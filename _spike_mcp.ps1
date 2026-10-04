# M1 spike: drive engram-mcp over stdio JSON-RPC
# verify initialize / tools/list / tools/call
# usage: powershell -File _spike_mcp.ps1  (run under G:\test1.x)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8

$exe = Join-Path $PSScriptRoot 'src-tauri\target\debug\engram-mcp.exe'
if (-not (Test-Path $exe)) { Write-Error "missing $exe, run: cargo build --bin engram-mcp"; exit 1 }

$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $exe
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.UseShellExecute = $false
$p = [System.Diagnostics.Process]::Start($psi)

function Send-Rpc($obj) {
    $json = $obj | ConvertTo-Json -Compress -Depth 10
    $p.StandardInput.WriteLine($json)
    $p.StandardInput.Flush()
}
function Read-Resp($label, $timeoutMs = 5000) {
    $task = $p.StandardOutput.ReadLineAsync()
    if ($task.Wait($timeoutMs)) {
        Write-Host "== $label =="
        Write-Host $task.Result
        return $task.Result
    } else {
        Write-Host "== $label == TIMEOUT, no response"
        return $null
    }
}

# 1. initialize handshake
Send-Rpc @{ jsonrpc = '2.0'; id = 1; method = 'initialize'; params = @{
    protocolVersion = '2025-11-25'
    capabilities    = @{}
    clientInfo      = @{ name = 'spike-test'; version = '0.1.0' }
} }
$r1 = Read-Resp 'initialize'

# 2. initialized notification
Send-Rpc @{ jsonrpc = '2.0'; method = 'notifications/initialized' }

# 3. tools/list
Send-Rpc @{ jsonrpc = '2.0'; id = 2; method = 'tools/list' }
$r2 = Read-Resp 'tools/list'

# 4. tools/call ping (with argument)
Send-Rpc @{ jsonrpc = '2.0'; id = 3; method = 'tools/call'; params = @{
    name      = 'ping'
    arguments = @{ text = 'spike-ok' }
} }
$r3 = Read-Resp 'tools/call ping'

try { $p.Kill() } catch {}

$pass = ($r1 -match '"serverInfo"') -and ($r2 -match '"ping"') -and ($r3 -match 'pong: spike-ok')
Write-Host ''
if ($pass) { Write-Host 'SPIKE PASS: handshake / tools-list / tools-call all green' }
else { Write-Host 'SPIKE FAIL: check output above' }
