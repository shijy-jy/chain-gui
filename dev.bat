@echo off
title Engram dev
cd /d "%~dp0"
echo ==========================================
echo  Engram dev launcher
echo  project dir: %CD%
echo ==========================================
echo.
echo Close this window to stop dev.
echo.
cargo tauri dev --config crates/engram-gui/tauri.conf.json
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo ===== ERROR =====
    pause
)
