# Dot-source this to put the project's Rust toolchain on PATH for the current PowerShell session:
#   . .\tools\env.ps1
# If your toolchain isn't on PATH (installed with --no-modify-path, say), create tools\env.local.ps1 (git-ignored)
# that sets $env:RUSTUP_HOME and $env:CARGO_HOME; nothing global is changed.
$local = Join-Path $PSScriptRoot 'env.local.ps1'
if (Test-Path $local) { . $local }
if ($env:CARGO_HOME -and -not ($env:Path -split ';' | Where-Object { $_ -eq "$env:CARGO_HOME\bin" })) {
    $env:Path = "$env:CARGO_HOME\bin;$env:Path"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Warning "cargo is not on PATH: install Rust from https://rustup.rs or set RUSTUP_HOME and CARGO_HOME in tools\env.local.ps1"
}
