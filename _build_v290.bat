@echo off
rem Engram 2.9.0 installer build (bundled embedding model, framework T11)
cd /d G:\test1.x
echo === BUILD START %DATE% %TIME% === > _build_v290.log
echo [1/4] cargo build --release -j 2 >> _build_v290.log 2>&1
cargo build --release -j 2 >> _build_v290.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
echo [2/4] sync engram-mcp.exe to resources >> _build_v290.log 2>&1
copy /y target\release\engram-mcp.exe crates\engram-gui\resources\engram-mcp.exe >> _build_v290.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
echo [3/4] sync local model to resources\models (bundle resources) >> _build_v290.log 2>&1
if exist crates\engram-gui\resources\models rmdir /s /q crates\engram-gui\resources\models
xcopy "%LOCALAPPDATA%\Engram\models" "crates\engram-gui\resources\models\" /E /I /Y >> _build_v290.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
echo [4/4] cargo tauri build from crates/engram-gui >> _build_v290.log 2>&1
cd /d G:\test1.x\crates\engram-gui
cargo tauri build >> G:\test1.x\_build_v290.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
cd /d G:\test1.x
echo === BUILD OK %DATE% %TIME% === >> _build_v290.log
goto end
:fail
echo === BUILD FAIL %DATE% %TIME% === >> _build_v290.log
:end
