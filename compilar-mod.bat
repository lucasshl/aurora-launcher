@echo off
chcp 65001 >nul
title Aurora Launcher - Compilar mod de cosmeticos 3D
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "scripts\build-mod.ps1"
echo.
pause
