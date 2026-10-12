@echo off
rem Starts the portable release build (run tools\build-release.ps1 once to create it).
set "DIR=%~dp0dist\EliteNexus"
if exist "%DIR%\EliteNexus.new.exe" (
  tasklist /FI "IMAGENAME eq EliteNexus.exe" 2>nul | find /I "EliteNexus.exe" >nul
  if errorlevel 1 move /Y "%DIR%\EliteNexus.new.exe" "%DIR%\EliteNexus.exe" >nul
)
if exist "%DIR%\EliteNexus.exe" (
  start "" "%DIR%\EliteNexus.exe"
) else (
  echo Release build not found. Building it now - this takes a few minutes the first time...
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\build-release.ps1" && start "" "%DIR%\EliteNexus.exe"
)
