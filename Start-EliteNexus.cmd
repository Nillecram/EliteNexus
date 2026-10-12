@echo off
rem Starts the portable release build (run tools\build-release.ps1 once to create it).
if exist "%~dp0dist\EliteNexus\EliteNexus.exe" (
  start "" "%~dp0dist\EliteNexus\EliteNexus.exe"
) else (
  echo Release build not found. Building it now - this takes a few minutes the first time...
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\build-release.ps1" && start "" "%~dp0dist\EliteNexus\EliteNexus.exe"
)
