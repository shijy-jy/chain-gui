@echo off
rem Engram 3.1.0 installer build (three-layer refactor + 3D graph + P2: human/AI-identical structure metrics,
rem analysis-mode hierarchy shells, overlay-aware framing, parallel cold-start scan)
cd /d G:\test1.x
echo === BUILD START %DATE% %TIME% === > _build_v310.log
echo [1/5] cargo build --release -j 4 >> _build_v310.log 2>&1
cargo build --release -j 4 >> _build_v310.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
echo [2/5] sync engram-mcp.exe to resources >> _build_v310.log 2>&1
copy /y target\release\engram-mcp.exe crates\engram-gui\resources\engram-mcp.exe >> _build_v310.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
echo [3/5] verify bundled embedding model >> _build_v310.log 2>&1
if not exist "crates\engram-gui\resources\models\bge-small-zh-v1.5\model_optimized.onnx" (
  if exist "%LOCALAPPDATA%\Engram\models" xcopy "%LOCALAPPDATA%\Engram\models" "crates\engram-gui\resources\models\" /E /I /Y >> _build_v310.log 2>&1
)
if not exist "crates\engram-gui\resources\models\bge-small-zh-v1.5\model_optimized.onnx" goto fail
echo [4/5] npm run build - frontend to dist >> _build_v310.log 2>&1
call npm.cmd run build >> _build_v310.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
echo [5/5] cargo tauri build from crates/engram-gui >> _build_v310.log 2>&1
cd /d G:\test1.x\crates\engram-gui
cargo tauri build >> G:\test1.x\_build_v310.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
cd /d G:\test1.x
echo === BUILD OK %DATE% %TIME% === >> _build_v310.log
goto end
:fail
cd /d G:\test1.x
echo === BUILD FAIL %DATE% %TIME% === >> _build_v310.log
:end
