@echo off
chcp 65001 >nul
title Aurora Launcher - Publicar atualizacao
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "scripts\release.ps1"
echo.
pause
