@echo off
chcp 65001 >nul
title Aurora Launcher - Corrigir Rust
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -Command ". '.\scripts\common.ps1'; Ensure-MsvcToolchain; Ok 'Rust pronto para compilar. Rode gerar-exe.bat de novo.'"
echo.
pause
