@echo off
rem Swaps in a newer Elite Nexus that tools\build-release.ps1 staged while the app was running.
rem Waits until Elite Nexus is closed, replaces the exe (your nexus-data is untouched), starts it.
set "DIR=%~dp0dist\EliteNexus"
if not exist "%DIR%\EliteNexus.new.exe" (
  echo No staged update found. Elite Nexus is up to date.
  goto :start
)
:wait
tasklist /FI "IMAGENAME eq EliteNexus.exe" 2>nul | find /I "EliteNexus.exe" >nul
if not errorlevel 1 (
  echo Waiting for Elite Nexus to close...
  timeout /t 2 /nobreak >nul
  goto :wait
)
move /Y "%DIR%\EliteNexus.new.exe" "%DIR%\EliteNexus.exe" >nul
echo Updated.
:start
start "" "%DIR%\EliteNexus.exe"
