@echo off
chcp 65001 >nul
title Aurora Launcher - Instalar
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "scripts\setup.ps1"
echo.
pause
