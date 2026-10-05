#!/usr/bin/env bash
# Repository-local human-test executable. Never installs system-wide.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p bin
# Mark the previously installed executable as stale while building. A failed build
# must not leave an older executable described as current.
if [ -f bin/BUILD.txt ]; then
    stamp=$(mktemp bin/.BUILD.XXXXXX)
    { printf 'STATUS: STALE — build in progress or failed\n'; cat bin/BUILD.txt; } > "$stamp"
    mv -f "$stamp" bin/BUILD.txt
fi
cargo build --release --locked -p tack-app
binary=$(mktemp bin/.tack.XXXXXX)
stamp=$(mktemp bin/.BUILD.XXXXXX)
trap 'rm -f "$binary" "$stamp"' EXIT
target_dir=$(cargo metadata --locked --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
cp "$target_dir/release/tack-app" "$binary"
chmod 755 "$binary"
dirty=false
if [ -n "$(git status --porcelain)" ]; then dirty=true; fi
{
    printf 'STATUS: CURRENT\n'
    printf 'commit: %s\n' "$(git rev-parse HEAD)"
    printf 'profile: release\nbuilt_utc: %s\nbinary: ./bin/tack\nworktree_dirty: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$dirty"
    printf 'sha256: %s\n' "$(sha256sum "$binary" | cut -d ' ' -f 1)"
    printf 'checkpoint: %s\n' "${1:-current workspace}"
} > "$stamp"
mv -f "$binary" bin/tack
mv -f "$stamp" bin/BUILD.txt
cat bin/BUILD.txt
