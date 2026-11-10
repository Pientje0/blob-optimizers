@echo off
setlocal
title Blob Optimizers
cd /d "%~dp0"

set "EXE=%~dp0src-tauri\target\release\blob-optimizers.exe"

rem Pass "rebuild" to force a fresh build:  "Run Blob Optimizers.bat" rebuild
if /i "%~1"=="rebuild" goto build
if exist "%EXE%" goto launch

:build
echo Blob Optimizers has not been built yet (or a rebuild was requested).
echo This takes a few minutes the first time.
echo.

where npm >nul 2>&1 || (
  echo [error] Node.js / npm was not found. Install Node.js 18+ from https://nodejs.org and try again.
  pause & exit /b 1
)
where cargo >nul 2>&1 || (
  echo [error] Rust / cargo was not found. Install Rust from https://rustup.rs and try again.
  pause & exit /b 1
)

if not exist "node_modules" (
  echo Installing dependencies...
  call npm install || (echo [error] npm install failed. & pause & exit /b 1)
)

echo Building Blob Optimizers...
call npx tauri build --no-bundle || (echo [error] Build failed. See the output above. & pause & exit /b 1)

if not exist "%EXE%" (
  echo [error] Build finished but "%EXE%" was not found.
  pause & exit /b 1
)

:launch
start "" "%EXE%"
exit /b 0
