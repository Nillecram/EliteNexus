# Off-screen UI screenshot of a debug build: the window opens at -20000,-20000 with no taskbar
# button, so it never appears over the game. Saves target\shots\<Out>.png and exits; never uploads.
#   powershell -File tools\dev\shot.ps1 -Page Exobiology -Tab untouched -Out bonus -Size 1300x900 -Delay 20
# -Page: a sidebar page (Dashboard, Exploration, Exobiology, Navigation, Market, ...).
# -Tab:  market sections (abnormal, database, ...), galaxy (whole-galaxy map), find / history /
#        unsold (Exploration), untouched / known (Exobiology > Find planets). find, untouched and
#        known start their search on the first frame, so give them -Delay 15-25 for Spansh.
#        With -Page Hud: a preset name (mocha, royalpurple, ...) to start the HUD colours page from.
# -Demo: a sample Ask conversation and an alert banner. -Exe: another build (e.g. an agent's own
#        CARGO_TARGET_DIR). The data folder is the build's own (target\debug\nexus-data).
param(
    [string]$Page = "Dashboard",
    [string]$Out = "shot",
    [string]$Size = "",
    [string]$Tab = "",
    [switch]$Demo,
    [double]$Delay = 0,
    [string]$Exe = "",
    [int]$TimeoutSec = 120
)
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if ($Exe -eq "") {
    $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
    $Exe = Join-Path $target 'debug\EliteNexus.exe'
}
[System.Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'BelowNormal'
$shots = Join-Path $root 'target\shots'
New-Item -ItemType Directory -Force -Path $shots | Out-Null
$png = Join-Path $shots "$Out.png"
if (Test-Path $png) { Remove-Item $png -Force -Confirm:$false }
$cmd = "--page `"$Page`" --screenshot `"$png`" --mute"
if ($Size -ne "") { $cmd += " --size $Size" }
if ($Tab -ne "") { $cmd += " --tab $Tab" }
if ($Demo) { $cmd += " --demo" }
if ($Delay -gt 0) { $cmd += " --shot-delay $Delay" }
$p = Start-Process -FilePath $Exe -ArgumentList $cmd -PassThru -WindowStyle Hidden
$null = $p.Handle
$p.PriorityClass = 'BelowNormal'
if (-not $p.WaitForExit($TimeoutSec * 1000)) {
    Stop-Process -Id $p.Id -Force -Confirm:$false
    Write-Output "TIMEOUT"
    exit 1
}
if (Test-Path $png) { Write-Output "saved $png" } else { Write-Output "NO PNG (exit $($p.ExitCode))"; exit 1 }
