# gen_bench_workspace.ps1 - 生成 N 节点基准工作区（dev 模式）
# 用法：powershell -File tools/gen_bench_workspace.ps1 -N 1000 -Out "G:\temp\bench-1000"
# 结构：hub 树 + 10% 孤立节点；用于扫描性能 / 布局 / 全库重嵌基准。
param([int]$N = 1000, [string]$Out = "")
if (-not $Out) { $Out = Join-Path $env:TEMP ("engram_bench_" + $N) }
New-Item -ItemType Directory -Force (Join-Path $Out ".chain\nodes") | Out-Null
Set-Content (Join-Path $Out ".chain\.mode") "dev" -NoNewline
$nodes = Join-Path $Out ".chain\nodes"
# 根 hub
Set-Content (Join-Path $nodes "hub-000.md") "---`nid: hub-000`ntype: note`ntitle: 基准根节点`ntags: [bench]`nparent: null`n---`n`n# 基准根节点`n"
$created = 0
# 每 20 个节点一个二级 hub，其余挂其下（树 + 扇出混合）
for ($i = 1; $i -le $N; $i++) {
  $id = "n-{0:D5}" -f $i
  if ($i % 20 -eq 1) { $parent = "hub-000" } else { $parent = "n-{0:D5}" -f (($i - (($i - 1) % 20)) ) }
  if ($i % 7 -eq 0) { $parent = "null" }   # 孤立节点
  $rel = if ($i % 3 -eq 0) { "solves" } else { "contains" }
  Set-Content (Join-Path $nodes "$id.md") "---`nid: $id`ntype: note`ntitle: 基准节点 $i`ntags: [bench]`nparent: $parent`nrel: $rel`n---`n`n# 基准节点 $i`n`n这是性能基准生成的节点正文，编号 $i。`n"
  $created++
}
"generated: $created nodes -> $Out"
