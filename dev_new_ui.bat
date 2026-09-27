@echo off
rem ============================================================
rem  看新版 UI（layout v3.0 / v3.1）—— 零等待验证
rem
rem  原理：debug 版 app.exe 走 devUrl(http://localhost:1420)，不内嵌前端，
rem        所以只要 vite 在跑，它显示的就是**当前源码**。
rem        release 版则把 dist 嵌进二进制，必须重构建才更新（这是"改了看不到"的原因）。
rem
rem  用法：双击本文件，或 npm run dev 之后运行它
rem ============================================================
title Engram dev (new UI)
cd /d "%~dp0"

powershell -NoProfile -Command "try { Invoke-WebRequest -Uri 'http://localhost:1420/' -TimeoutSec 3 -UseBasicParsing | Out-Null; exit 0 } catch { exit 1 }"
if %ERRORLEVEL% NEQ 0 (
  echo [1/2] vite dev server 未启动，正在拉起...
  start "Engram vite" cmd /c "npm.cmd run dev"
  echo       等待 vite 就绪（约 6 秒）...
  timeout /t 6 /nobreak >NUL
) else (
  echo [1/2] vite dev server 已在线（复用）
)

if not exist "target\debug\app.exe" (
  echo [2/2] 缺少 target\debug\app.exe —— 先跑一次: cargo build
  pause
  exit /b 1
)

echo [2/2] 启动 debug 版应用（显示当前源码构建的前端）
echo.
echo   关闭本窗口不会关掉应用，请直接关应用的窗口
echo.
"target\debug\app.exe"
