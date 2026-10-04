$p = "G:\test1.x\src\App.svelte"
$lines = Get-Content -LiteralPath $p -Encoding UTF8
# 把"只清类、不重置聚焦状态"的调用点统一成 clearFocus()（状态不一致会让标签优先级与视觉层次脱节）
$pat = "cyRef.elements().removeClass('focus-dim focus-lit');"
$helper = @'
  /** 统一退出聚焦：清类 + 重置状态 + 重算标签（三者必须一起做，否则视觉层次与标签优先级脱节） */
  function clearFocus(cyRef: Core | null) {
    focusNodeId = null;
    focusSet = null;
    if (cyRef) cyRef.elements().removeClass('focus-dim focus-lit');
    scheduleLabelUpdate(200);
  }

'@
$out = New-Object System.Collections.Generic.List[string]
$inserted = $false
foreach ($l in $lines) {
  if (-not $inserted -and $l -match '^\s*/\*\*\s*$' -and $out.Count -gt 0 -and $out[-1] -match 'applyFocusClasses') {
    $out.Add($helper.TrimEnd())
    $inserted = $true
  }
  $out.Add($l)
}
Set-Content -LiteralPath $p -Value $out -Encoding UTF8
"插入 clearFocus: $inserted"
Select-String -LiteralPath $p -Pattern "function clearFocus" | ForEach-Object { "$($_.LineNumber): $($_.Line.Trim())" }
