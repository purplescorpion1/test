#!/usr/bin/env bash
# build_win.sh — Cross-compile Lemmings for Windows 11 x64
# Run this on Linux/macOS with mingw-w64 or from Windows PowerShell with MSVC.
#
# On Windows (PowerShell):
#   .\build_win.sh   — or just: cargo build --release
#
# On Linux (cross-compile):
#   sudo apt install mingw-w64 libsdl2-dev
#   cargo install cross
#   cross build --release --target x86_64-pc-windows-gnu

set -euo pipefail

TARGET="x86_64-pc-windows-msvc"
PROFILE="release"

echo "[build_win] Installing target if needed..."
rustup target add "$TARGET" 2>/dev/null || true

echo "[build_win] Building lemmings $PROFILE for $TARGET..."
cargo build --$PROFILE --target "$TARGET"

EXE="target/$TARGET/$PROFILE/lemmings.exe"
if [ -f "$EXE" ]; then
    echo "[build_win] SUCCESS: $EXE"
    echo "[build_win] Size: $(du -sh "$EXE" | cut -f1)"
else
    echo "[build_win] FAILED — exe not found"
    exit 1
fi

echo ""
echo "=== Deploy checklist ==="
echo "Copy to your game folder:"
echo "  $EXE"
echo "  SDL2.dll              (from SDL2-2.30.x-win32-x64.zip)"
echo "  SDL2_mixer.dll        (from SDL2_mixer-2.8.x-win32-x64.zip)"
echo "  Lemmings.1of2.adf     (your disk image)"
echo "  Lemmings.2of2.adf     (your disk image)"
echo ""
echo "=== Build installer ==="
echo "  cd installer && makensis lemmings.nsi"
echo "  -> installer/LemmingsSetup.exe"
