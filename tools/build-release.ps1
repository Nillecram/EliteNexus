# Builds an optimised Elite Nexus and assembles a portable folder in .\dist\EliteNexus
#   powershell -ExecutionPolicy Bypass -File .\tools\build-release.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'env.ps1')
Set-Location $root

cargo build --release -p nexus-app
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$dist = Join-Path $root 'dist\EliteNexus'
New-Item -ItemType Directory -Force -Path $dist | Out-Null
Copy-Item (Join-Path $root 'target\release\EliteNexus.exe') $dist -Force
Copy-Item (Join-Path $root 'README.md') $dist -Force
Copy-Item (Join-Path $root 'NOTICE.md') $dist -Force
Copy-Item (Join-Path $root 'docs\USER_GUIDE.md') $dist -Force
# Keep user data (nexus-data) if it already exists next to the exe.
$exe = Get-Item (Join-Path $dist 'EliteNexus.exe')
"Built {0} ({1:N1} MB) -> {2}" -f $exe.Name, ($exe.Length / 1MB), $dist
