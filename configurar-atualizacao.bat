@echo off
chcp 65001 >nul
title Aurora Launcher - Configurar atualizacao
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "scripts\setup-updater.ps1"
echo.
pause
