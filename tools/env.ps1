# Dot-source this to put the project's Rust toolchain on PATH for the current PowerShell session:
#   . .\tools\env.ps1
# The toolchain lives on D: (installed with --no-modify-path), so nothing global is changed.
$env:RUSTUP_HOME = "D:\GameDev\Tools\Rust\rustup"
$env:CARGO_HOME  = "D:\GameDev\Tools\Rust\cargo"
if (-not ($env:Path -split ';' | Where-Object { $_ -eq "$env:CARGO_HOME\bin" })) {
    $env:Path = "$env:CARGO_HOME\bin;$env:Path"
}
