$p = "G:\test1.x\src\App.svelte"
$lines = Get-Content -LiteralPath $p -Encoding UTF8
# 删除标签系统（72..297 行，1-based），保留 lastCamSig（布局参数 effect 仍在用）
$head = $lines[0..69]        # 1..70
$tail = $lines[297..($lines.Count - 1)]  # 298..end
$keep = @(
  '',
  '  /** 上一次布局参数签名（避免重复重排） */',
  "  let lastCamSig = '';",
  ''
)
Set-Content -LiteralPath $p -Value ($head + $keep + $tail) -Encoding UTF8
"新行数: " + (Get-Content -LiteralPath $p).Count
Select-String -LiteralPath $p -Pattern "lastCamSig" | ForEach-Object { "$($_.LineNumber): $($_.Line.Trim())" }
