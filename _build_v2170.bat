@echo off
rem Engram 2.17.0 installer build (reading mode: graph view <-> node file tree, human-only view)
cd /d G:\test1.x
echo === BUILD START %DATE% %TIME% === > _build_v2170.log
echo [1/5] cargo build --release -j 2 >> _build_v2170.log 2>&1
cargo build --release -j 2 >> _build_v2170.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
echo [2/5] sync engram-mcp.exe to resources >> _build_v2170.log 2>&1
copy /y target\release\engram-mcp.exe crates\engram-gui\resources\engram-mcp.exe >> _build_v2170.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
echo [3/5] verify bundled embedding model >> _build_v2170.log 2>&1
if not exist "crates\engram-gui\resources\models\bge-small-zh-v1.5\model_optimized.onnx" (
  if exist "%LOCALAPPDATA%\Engram\models" xcopy "%LOCALAPPDATA%\Engram\models" "crates\engram-gui\resources\models\" /E /I /Y >> _build_v2170.log 2>&1
)
if not exist "crates\engram-gui\resources\models\bge-small-zh-v1.5\model_optimized.onnx" goto fail
echo [4/5] npm run build - frontend to dist >> _build_v2170.log 2>&1
call npm.cmd run build >> _build_v2170.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
echo [5/5] cargo tauri build from crates/engram-gui >> _build_v2170.log 2>&1
cd /d G:\test1.x\crates\engram-gui
cargo tauri build >> G:\test1.x\_build_v2170.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
cd /d G:\test1.x
echo === BUILD OK %DATE% %TIME% === >> _build_v2170.log
goto end
:fail
cd /d G:\test1.x
echo === BUILD FAIL %DATE% %TIME% === >> _build_v2170.log
:end
