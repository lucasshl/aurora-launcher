@echo off
chcp 65001 >nul
title Aurora Launcher - Modo teste
cd /d "%~dp0"
if not exist node_modules call instalar.bat
if exist .keys\ms-client-id.txt set /p AURORA_MS_CLIENT_ID=<.keys\ms-client-id.txt
call npm run tauri dev
pause
