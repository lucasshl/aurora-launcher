@echo off
chcp 65001 >nul
title Aurora Launcher - Subir para o GitHub
cd /d "%~dp0"
set "MSG="
set /p "MSG=Descreva a mudança (ou só Enter): "
if not defined MSG goto semmsg
powershell -NoProfile -ExecutionPolicy Bypass -File "scripts\push-github.ps1" -Message "%MSG%"
goto fim
:semmsg
powershell -NoProfile -ExecutionPolicy Bypass -File "scripts\push-github.ps1"
:fim
echo.
pause
