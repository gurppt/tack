#!/usr/bin/env bash
# Optional app-local libXi for X11 distributions affected by the startup crash.
set -euo pipefail
cd -- "$(dirname -- "$0")/.."
project_root=$(pwd -P)
if [ "$(uname -s)" != Linux ]; then
    echo "This helper is only needed on Linux/X11." >&2
    exit 1
fi
for prerequisite in curl sha256sum tar make cc pkg-config; do
    if ! command -v "$prerequisite" >/dev/null; then
        echo "Missing build prerequisite: $prerequisite" >&2
        exit 1
    fi
done
prefix="$project_root/target/native/libXi-1.8.3"
work=$(mktemp -d /tmp/tack-libXi-build.XXXXXXXX)
trap 'rm -rf -- "$work"' EXIT
archive="$work/libXi-1.8.3.tar.xz"
curl --fail --location --show-error --silent --max-time 120 \
    https://www.x.org/releases/individual/lib/libXi-1.8.3.tar.xz --output "$archive"
printf '%s  %s\n' \
    7ad60056f01af4f786cfe93b3a7707447711626fc8da2637bec71a90409babe5 "$archive" \
    | sha256sum --check --status
tar -xf "$archive" -C "$work"
cd -- "$work/libXi-1.8.3"
./configure --prefix="$prefix" --disable-static --disable-docs
make -j2
make install
cp COPYING "$prefix/COPYING"
printf 'Installed app-local libXi 1.8.3 in %s\nRun cargo run --release --locked -p tack-app from the project.\n' "$prefix"
