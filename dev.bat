@echo off
title chain-gui dev
cd /d "%~dp0"
echo ==========================================
echo  chain-gui dev launcher
echo  project dir: %CD%
echo ==========================================
echo.
echo Close this window to stop dev.
echo.
cargo tauri dev
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo ===== ERROR =====
    pause
)
