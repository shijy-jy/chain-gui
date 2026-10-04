$ErrorActionPreference = "Stop"
# 脱离会话的独立构建：显式 PATH（脱壳进程不继承交互会话的 PATH 修改）
$env:PATH = "C:\Users\jcm20\.cargo\bin;G:\nodejs22\node-v22.19.0-win-x64;$env:PATH"
Set-Location G:\test1.x
$log = "G:\test1.x\_build_v2110.log"
function Log($m) { Add-Content -Path $log -Value $m -Encoding UTF8 }
Log "=== BUILD START $(Get-Date -Format o) ==="
function Step($name, $script) {
  Log "--- $name ---"
  # 原生命令 stderr（cargo 进度行）在 EAP=Stop 下会被 *>> 重定向成异常——调用期降为 Continue，只看退出码
  $ErrorActionPreference = "Continue"
  & $script *>> $log
  $code = $LASTEXITCODE
  $ErrorActionPreference = "Stop"
  if ($code -ne 0) { Log "=== FAIL $name : exit code $code ==="; exit 1 }
}
Step "cargo release" { cargo build --release -j 2 }
Step "sync mcp" { Copy-Item "target\release\engram-mcp.exe" "crates\engram-gui\resources\engram-mcp.exe" -Force }
Step "sync model" {
  if (Test-Path "crates\engram-gui\resources\models") { Remove-Item "crates\engram-gui\resources\models" -Recurse -Force }
  Copy-Item "$env:LOCALAPPDATA\Engram\models" "crates\engram-gui\resources\models" -Recurse -Force
}
Step "npm build (frontend to dist)" { npm.cmd run build }
Step "tauri build" {
  Set-Location "crates\engram-gui"
  cargo tauri build
}
Log "=== BUILD OK $(Get-Date -Format o) ==="
