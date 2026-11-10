@echo off
chcp 65001 >nul
title Aurora Launcher - Gerar EXE
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "scripts\build.ps1"
echo.
pause
