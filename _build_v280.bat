@echo off
cd /d G:\test1.x
echo === BUILD START %DATE% %TIME% === > _build_v280.log
echo [1/3] cargo build --release -j 2 >> _build_v280.log 2>&1
cargo build --release -j 2 >> _build_v280.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
echo [2/3] sync engram-mcp.exe to resources >> _build_v280.log 2>&1
copy /y target\release\engram-mcp.exe crates\engram-gui\resources\engram-mcp.exe >> _build_v280.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
echo [3/3] cargo tauri build from crates/engram-gui >> _build_v280.log 2>&1
cd /d G:\test1.x\crates\engram-gui
cargo tauri build >> G:\test1.x\_build_v280.log 2>&1
if %ERRORLEVEL% NEQ 0 goto fail
cd /d G:\test1.x
echo === BUILD OK %DATE% %TIME% === >> _build_v280.log
goto end
:fail
echo === BUILD FAIL %DATE% %TIME% === >> _build_v280.log
:end
