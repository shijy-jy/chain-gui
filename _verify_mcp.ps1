# _verify_mcp.ps1 - engram-mcp end-to-end verification over stdio JSON-RPC
# Temp workspace (dev mode): handshake instructions (D4) / tools-list (9) / all 9 tools /
# optimistic-lock CONFLICT (D3) / strict rel_type (D2) / duplicate-title guard.
# ASCII only (PS 5.1 parses UTF-8-no-BOM scripts as GBK and breaks CJK literals).

$ErrorActionPreference = "Stop"
$exe = "G:\test1.x\src-tauri\target\debug\engram-mcp.exe"
$ws  = Join-Path $env:TEMP ("engram_mcp_verify_" + $PID)
$script:step = 0
$script:reqId = 0
$proc = $null

function Read-Line($timeoutMs = 8000) {
    $task = $proc.StandardOutput.ReadLineAsync()
    if (-not $task.Wait($timeoutMs)) { throw "TIMEOUT waiting response line" }
    return $task.Result
}

function Send-Req($method, $paramsJson) {
    $script:reqId++
    $json = '{"jsonrpc":"2.0","id":' + $script:reqId + ',"method":"' + $method + '","params":' + $paramsJson + '}'
    $proc.StandardInput.WriteLine($json)
    $proc.StandardInput.Flush()
    return Read-Line 8000
}

function Call-Tool($name, $argsJson) {
    return Send-Req "tools/call" ('{"name":"' + $name + '","arguments":' + $argsJson + '}')
}

function Check($name, $resp, $pattern) {
    $script:step++
    # tool results nest the payload JSON inside content[0].text with escaped quotes;
    # unescape \" so patterns can be written against plain JSON
    $norm = $resp -replace '\\"', '"'
    if ($norm -notmatch $pattern) {
        Write-Host "VERIFY FAIL [$($script:step)] $name"
        Write-Host "  pattern : $pattern"
        Write-Host "  response: $resp"
        throw "VERIFY FAILED at step $($script:step): $name"
    }
    Write-Host "ok [$($script:step)] $name"
}

try {
    if (-not (Test-Path $exe)) { throw "binary not found: $exe (build first)" }

    # --- temp workspace: .chain/.mode=dev + two seed nodes (UTF-8 no BOM via .NET) ---
    [System.IO.Directory]::CreateDirectory((Join-Path $ws ".chain\nodes")) | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $ws ".chain\.mode"), "dev")
    $seedA = @'
---
id: seed-a
type: note
title: Seed Alpha
parent: null
status: none
created: 2026-09-01T10:00:00+08:00
updated: 2026-09-01T10:00:00+08:00
revision: 1
tags: []
---

Seed alpha body.
'@
    $seedB = $seedA -replace 'seed-a', 'seed-b' -replace 'Seed Alpha', 'Seed Beta'
    [System.IO.File]::WriteAllText((Join-Path $ws ".chain\nodes\seed-a.md"), $seedA)
    [System.IO.File]::WriteAllText((Join-Path $ws ".chain\nodes\seed-b.md"), $seedB)

    # --- launch server ---
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.Arguments = '--workspace "' + $ws + '"'
    $psi.RedirectStandardInput  = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError  = $true
    $psi.UseShellExecute = $false
    $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $proc = [System.Diagnostics.Process]::Start($psi)

    # 1) initialize: instructions carry guide version + write rules (D4)
    $r = Send-Req "initialize" '{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"verify","version":"0.1"}}'
    Check "handshake serverInfo engram-mcp" $r '"name"\s*:\s*"engram-mcp"'
    Check "handshake instructions banner" $r 'Engram MCP server'
    Check "instructions mention get_guide" $r 'get_guide'
    Check "instructions mention expected_updated" $r 'expected_updated'

    # 2) tools/list: the v1 nine tools
    $r = Send-Req "tools/list" '{}'
    foreach ($t in @("get_overview","search","read_node","expand","read_path","get_guide","create_node","update_node","link_nodes")) {
        Check "tool listed: $t" $r ('"name"\s*:\s*"' + $t + '"')
    }

    # 3) create_node
    $r = Call-Tool "create_node" '{"title":"Memory Alpha","body":"alpha body","tags":["mcp"]}'
    Check "create_node created" $r '"created"\s*:\s*true'
    Check "create_node auto id node-1" $r '"id"\s*:\s*"node-1"'

    # 4) duplicate title guard (case-insensitive), force passes
    $r = Call-Tool "create_node" '{"title":"memory alpha"}'
    Check "duplicate title rejected" $r 'DUPLICATE_TITLE'
    $r = Call-Tool "create_node" '{"title":"memory alpha","force":true}'
    Check "duplicate title force passes" $r '"created"\s*:\s*true'

    # 5) search
    $r = Call-Tool "search" '{"query":"alpha body"}'
    Check "search hits" $r '"total"\s*:\s*[1-9]'

    # 6) read_node with neighbors
    $r = Call-Tool "read_node" '{"id":"node-1","include_neighbors":true}'
    Check "read_node body" $r 'alpha body'
    Check "read_node neighbors" $r 'neighbors'

    # 7) link_nodes + strict rel_type (D2)
    $r = Call-Tool "link_nodes" '{"from":"seed-a","to":"node-1","rel_type":"contains","desc":"test edge"}'
    Check "link_nodes ok" $r '"isError"\s*:\s*false'
    $r = Call-Tool "link_nodes" '{"from":"seed-a","to":"node-1","rel_type":"depends_on"}'
    Check "bad rel_type rejected (D2)" $r 'rel_type'

    # 8) expand
    $r = Call-Tool "expand" '{"id":"seed-a","depth":1}'
    Check "expand sees neighbor" $r '"node_count"\s*:\s*[2-9]'

    # 9) read_path narrative
    $r = Call-Tool "read_path" '{"from":"node-1","to":"seed-a"}'
    Check "read_path found" $r '"found"\s*:\s*true'
    Check "read_path narrative rel" $r 'contains'

    # 10) update_node append + optimistic lock (D3)
    $r = Call-Tool "update_node" '{"id":"node-1","mode":"append","content":"appended line"}'
    Check "update append ok" $r '"updated"\s*:\s*true'
    $u1 = [regex]::Match(($r -replace '\\"', '"'), '"updated_at"\s*:\s*"([^"]+)"').Groups[1].Value
    if (-not $u1) { throw "updated_at not found in update response: $r" }
    $r = Call-Tool "update_node" '{"id":"node-1","mode":"replace_body","content":"evil overwrite","expected_updated":"2000-01-01T00:00:00+08:00"}'
    Check "stale lock CONFLICT (D3)" $r 'CONFLICT'
    $r = Call-Tool "update_node" ('{"id":"node-1","mode":"append","content":"safe append","expected_updated":"' + $u1 + '"}')
    Check "fresh lock passes (D3)" $r '"updated"\s*:\s*true'

    # 11) get_guide / get_overview
    $r = Call-Tool "get_guide" '{}'
    Check "get_guide version 2 (dev)" $r '"version"\s*:\s*2'
    Check "get_guide content" $r '"content"\s*:'
    $r = Call-Tool "get_overview" '{}'
    Check "overview node_count 4" $r '"node_count"\s*:\s*4'

    Write-Host ""
    Write-Host "VERIFY PASS: all $($script:step) checks green"
}
finally {
    if ($proc) {
        try { $proc.StandardInput.Close() } catch {}
        try { if (-not $proc.WaitForExit(2000)) { $proc.Kill() } } catch {}
        $proc.Dispose()
    }
    Remove-Item $ws -Recurse -Force -ErrorAction SilentlyContinue
}
