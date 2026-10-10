#!/usr/bin/env bash
# Explicit cross-build. Prepared pinned libjpeg-turbo must include SIMD/NASM.
set -euo pipefail
cd "$(dirname "$0")/.."
free_bytes=$(df -B1 --output=avail . | tail -n 1)
if ((free_bytes < 12*1024*1024*1024)); then
    printf 'Windows build needs 12 GiB free to retain the 10 GiB reserve.\n' >&2
    exit 1
fi
native="$PWD/target/native/libjpeg-turbo-3.2.0-windows-gnu"
if [ ! -f "$native/native-build.json" ]; then
    printf 'First run: python3 tools/prepare_turbojpeg.py --target windows-gnu (NASM on PATH).\n' >&2
    exit 1
fi
export TACK_JPEG_NATIVE_ROOT="$native"
export TURBOJPEG_LIB_DIR="$native/lib"
export TURBOJPEG_INCLUDE_DIR="$native/include"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
cargo build --target x86_64-pc-windows-gnu --release --locked -p tack-app -p tack-server
python3 tools/package_windows.py
