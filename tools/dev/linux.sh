#!/bin/bash
# Elite Nexus on Linux, from a Windows PC: runs inside a WSL distro with Rust, Xvfb and Mesa (AGENTS.md > Linux).
# Run from the repository root in PowerShell:
#   wsl -d EliteNexusLinux -u root -- bash tools/dev/linux.sh cargo clippy --workspace --all-targets -j 4 -- -D warnings
#   wsl -d EliteNexusLinux -u root -- bash tools/dev/linux.sh cargo test --workspace -j 4
#   wsl -d EliteNexusLinux -u root -- bash tools/dev/linux.sh shot --page Settings --out linux-settings --size 1000x760
# Builds go to /root/target inside the distro, never target\ (Windows and Linux builds would fight over it), at the
# lowest priority. A shot builds the debug app, renders it under Xvfb (no window anywhere), saves
# target\shots\<out>.png and exits; like every screenshot run it never uploads.
set -u
cd "$(dirname "$0")/../.." || exit 1
export CARGO_TARGET_DIR=/root/target PATH="/root/.cargo/bin:$PATH" CARGO_TERM_COLOR=never

case "${1:-}" in
  cargo)
    shift
    exec nice -n 19 cargo "$@"
    ;;
  shot)
    shift
    page=Dashboard; out=linux-shot; size=1000x760; delay=2; extra=()
    while [ $# -gt 0 ]; do
      case "$1" in
        --page) page=$2; shift 2 ;;
        --tab) extra+=(--tab "$2"); shift 2 ;;
        --out) out=$2; shift 2 ;;
        --size) size=$2; shift 2 ;;
        --delay) delay=$2; shift 2 ;;
        --journal-dir) extra+=(--journal-dir "$2"); shift 2 ;;
        --demo) extra+=(--demo); shift ;;
        *) echo "unknown option $1"; exit 2 ;;
      esac
    done
    nice -n 19 cargo build -p nexus-app -j 4 || exit 1
    mkdir -p target/shots
    # Software OpenGL (Mesa's llvmpipe) on a virtual X display: Mesa's software Vulkan crashes under WSL1.
    export WGPU_BACKEND=gl
    exec xvfb-run -a -s "-screen 0 1920x1080x24" nice -n 19 "$CARGO_TARGET_DIR/debug/EliteNexus" --page "$page" "${extra[@]}" \
      --screenshot "target/shots/$out.png" --size "$size" --shot-delay "$delay" --mute
    ;;
  *)
    echo "usage: linux.sh cargo <args...> | linux.sh shot [--page P] [--tab T] [--out NAME] [--size WxH] [--delay S] [--journal-dir D] [--demo]"
    exit 2
    ;;
esac
