# 节点紧凑摘要：id | title | tags | 正文首行（起草触发句用）
$wss = @("G:\learning","G:\deepseek\alive_data","G:\ta","G:\story\story","G:\water","G:\openGL\render_unified_oss","G:\test1.x\demo\dev","G:\test1.x\demo\analysis")
foreach ($ws in $wss) {
  "==== $ws"
  $nodes = Join-Path $ws ".chain\nodes"
  if (-not (Test-Path $nodes)) { continue }
  foreach ($f in (Get-ChildItem $nodes -Filter *.md -File | Sort-Object Name)) {
    $c = [System.IO.File]::ReadAllText($f.FullName, [System.Text.Encoding]::UTF8)
    $id = if ($c -match "(?m)^id:\s*(\S+)") { $matches[1] } else { $f.BaseName }
    $title = if ($c -match "(?m)^title:\s*(.+)$") { $matches[1].Trim() } else { $f.BaseName }
    $tags = if ($c -match "(?m)^tags:\s*\[([^\]]*)\]") { $matches[1].Trim() } else { "" }
    $hasTrig = $c -match ">\s*触发"
    # 正文首行（frontmatter 之后第一个非空、非 # 标题行）
    $body = ($c -split "`r?`n---`r?`n", 2)[1]
    $first = ""
    if ($body) {
      foreach ($line in ($body -split "`r?`n")) {
        $t = $line.Trim()
        if ($t -ne "" -and -not $t.StartsWith("#") -and -not $t.StartsWith(">")) { $first = $t; break }
      }
    }
    $flag = if ($hasTrig) { "" } else { " [缺]" }
    "[{0}] {1} | {2} | {3}{4}" -f $id, $title, $tags, $first.Substring(0, [math]::Min(60, $first.Length)), $flag
  }
}
