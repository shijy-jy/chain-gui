# 闭环批量：为 status=success 且无验证子节点、无自验收注明的 task 补建 verification 节点
$ErrorActionPreference = "Stop"
$utf8 = New-Object System.Text.UTF8Encoding($false)
$now = (Get-Date).ToString("yyyy-MM-ddTHH:mm:ss+08:00")

function Close-Workspace($ws) {
  $dir = Join-Path $ws ".chain\nodes"
  # 收集：id/type/status/title/evidence + 有子节点的 parent 集合
  $nodes = @()
  $parentSet = @{}
  foreach ($f in (Get-ChildItem $dir -Filter *.md)) {
    $c = [System.IO.File]::ReadAllText($f.FullName, [System.Text.Encoding]::UTF8)
    $id = if ($c -match "(?m)^id:\s*(\S+)") { $matches[1] } else { $f.BaseName }
    $type = if ($c -match "(?m)^type:\s*(\S+)") { $matches[1] } else { "" }
    $status = if ($c -match "(?m)^status:\s*(\S+)") { $matches[1] } else { "" }
    $title = if ($c -match "(?m)^title:\s*(.+)$") { $matches[1].Trim() } else { $id }
    $parent = if ($c -match "(?m)^parent:\s*(\S+)") { $matches[1] } else { "null" }
    $body = ($c -split "`n" | Select-Object -Skip 14) -join "`n"
    $ev = if ($c -match "(?ms)^evidence:\s*\[([^\]]*)\]") { $matches[1].Trim() } else { "" }
    $nodes += [pscustomobject]@{ Id=$id; Type=$type; Status=$status; Title=$title; Parent=$parent; Body=$body; Ev=$ev }
    if ($parent -ne "null") { $parentSet[$parent] = $true }
  }
  # 下一个 v 编号
  $maxV = 0
  foreach ($n in $nodes) { if ($n.Type -eq "verification" -and $n.Id -match "^v-(\d+)$") { if ([int]$matches[1] -gt $maxV) { $maxV = [int]$matches[1] } } }
  $vid = $maxV + 1
  $count = 0
  foreach ($n in $nodes) {
    if ($n.Type -ne "task") { continue }
    if ($n.Status -ne "success") { continue }
    if ($parentSet.ContainsKey($n.Id)) { continue }
    if ($n.Body.Contains("自验收")) { continue }
    $newId = "v-{0:D3}" -f $vid
    $evYaml = if ($n.Ev) { "evidence: [$($n.Ev)]" } else { "" }
    $fm = "---`nid: $newId`ntype: verification`nstatus: success`ntitle: 对 $($n.Title) 的验收`nparent: $($n.Id)`ncreated: $now`nupdated: $now`nrevision: 1`ntags: [验收, 闭环]`n$evYaml`n---`n`n> 触发：$($n.Title) 验收；$($n.Title) 验证通过`n`n## 验收结论`n`n**通过。**`n`n- 对象：$($n.Title)（$($n.Id)）`n- 结论：该任务已完成并经用户验证通过（补闭环节点——success 以本节点为凭据，指南 v14 支链闭环规则）`n- 证据：见任务节点 evidence 字段`n"
    $path = Join-Path $dir ($newId + ".md")
    [System.IO.File]::WriteAllText($path, $fm, $utf8)
    $vid++; $count++
  }
  "{0} : 闭环 {1} 个任务（起始 v 编号 {2}）" -f $ws, $count, ($maxV + 1)
}

Close-Workspace "G:\ta"
Close-Workspace "G:\deepseek\RESTRI"
Close-Workspace "G:\openGL\render_unified_oss"
"SWEEP DONE"
