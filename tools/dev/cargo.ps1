# Runs cargo at BelowNormal priority with the project toolchain: the game is often running on this
# machine, so builds must never take the CPU from it. Output goes to target\cc\<Log>.log.
#   powershell -File tools\dev\cargo.ps1 -Log build -CargoArgs "build -p nexus-app -j 4"
#   powershell -File tools\dev\cargo.ps1 -Log test -TargetDir target-agents\me -CargoArgs "test --workspace -j 4"
param(
    [string]$TargetDir = "",
    [string]$Log = "last-cargo",
    [string]$CargoArgs = ""
)
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $root 'tools\env.ps1')
[System.Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'BelowNormal'
Set-Location $root
if ($TargetDir -ne "") { $env:CARGO_TARGET_DIR = $TargetDir }
$env:CARGO_TERM_COLOR = "never"
$logs = Join-Path $root 'target\cc'
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$out = Join-Path $logs "$Log.log"
$p = Start-Process -FilePath cargo -ArgumentList $CargoArgs -NoNewWindow -PassThru -RedirectStandardError "$out.err" -RedirectStandardOutput "$out.out"
$null = $p.Handle
$p.PriorityClass = 'BelowNormal'
$p.WaitForExit()
Get-Content "$out.out", "$out.err" | Set-Content -Encoding utf8 $out
Remove-Item "$out.out", "$out.err" -ErrorAction SilentlyContinue
exit $p.ExitCode
