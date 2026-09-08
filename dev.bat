@echo off
title Engram dev
cd /d "%~dp0"
echo ==========================================
echo  Engram dev launcher
echo  project dir: %CD%
echo ==========================================
echo.
echo Close both windows to stop dev.
echo.
rem 前端 dev server 单独拉起（tauri.conf 不再依赖 beforeDevCommand，cwd 歧义根治）
start "Engram vite" cmd /c "npm.cmd run dev"
rem 等 vite 起来再启动 tauri
timeout /t 3 /nobreak >NUL
cargo tauri dev --config crates/engram-gui/tauri.conf.json
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo ===== ERROR =====
    pause
)
