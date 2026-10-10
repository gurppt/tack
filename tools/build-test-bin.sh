#!/usr/bin/env bash
# Repository-local human-test executable. Never installs system-wide.
set -euo pipefail
cd "$(dirname "$0")/.."
# Runtime icons are editable user artwork. Seed missing files only.
python3 tools/install_toolbar_icons.py gfx/icons bin/gfx/icons
# Mark the previously installed executable as stale while building. A failed build
# must not leave an older executable described as current.
if [ -f bin/BUILD.txt ]; then
    stamp=$(mktemp bin/.BUILD.XXXXXX)
    { printf 'STATUS: STALE — build in progress or failed\n'; cat bin/BUILD.txt; } > "$stamp"
    mv -f "$stamp" bin/BUILD.txt
fi
cargo build --release --locked -p tack-app -p tack-server
binary=$(mktemp bin/.tack.XXXXXX)
server=$(mktemp bin/.tack-server.XXXXXX)
stamp=$(mktemp bin/.BUILD.XXXXXX)
artwork=$(mktemp bin/.about.XXXXXX)
decoder=$(mktemp bin/.jpeg.XXXXXX)
trap 'rm -f "$binary" "$server" "$stamp" "$artwork" "$decoder"' EXIT
target_dir=$(cargo metadata --locked --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
cp "$target_dir/release/tack-app" "$binary"
chmod 755 "$binary"
cp "$target_dir/release/tack-server" "$server"
chmod 755 "$server"
# Keep the human build compact; symbol-rich originals remain in shared target/.
if command -v strip >/dev/null 2>&1; then
    strip --strip-all "$binary" "$server"
fi
cp "$target_dir/release/tack-about-logo.png" bin/tack-about-logo.png
cp "$target_dir/release/tack-about.png" "$artwork"
cp "$target_dir/release/tack-jpeg-decoder" "$decoder"
chmod 755 "$decoder"
for icon in "$target_dir"/release/tack-icon-*.png "$target_dir"/release/tack-icon.ico; do
    cp "$icon" bin/
done
python3 - <<'PYICON'
from pathlib import Path
root=Path.cwd()/"bin"
(root/"tack.desktop").write_text("[Desktop Entry]\nType=Application\nName=Tack\nComment=Reference boards\nExec=\""+str(root/"tack")+"\"\nIcon="+str(root/"tack-icon-64.png")+"\nTerminal=false\nCategories=Graphics;\n")
PYICON
dirty=false
if [ -n "$(git status --porcelain)" ]; then dirty=true; fi
{
    printf 'STATUS: CURRENT\n'
    printf 'commit: %s\n' "$(git rev-parse HEAD)"
    printf 'profile: release\nbuilt_utc: %s\nbinary: ./bin/tack\nserver_binary: ./bin/tack-server\nworktree_dirty: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$dirty"
    printf 'sha256: %s\n' "$(sha256sum "$binary" | cut -d ' ' -f 1)"
    printf 'server_sha256: %s\n' "$(sha256sum "$server" | cut -d ' ' -f 1)"
    printf 'about_asset_sha256: %s\n' "$(sha256sum "$artwork" | cut -d ' ' -f 1)"
    printf 'jpeg_decoder_sha256: %s\n' "$(sha256sum "$decoder" | cut -d ' ' -f 1)"
    printf 'checkpoint: %s\n' "${1:-current workspace}"
} > "$stamp"
mv -f "$artwork" bin/tack-about.png
mv -f "$decoder" bin/tack-jpeg-decoder
mv -f "$binary" bin/tack
mv -f "$server" bin/tack-server
mv -f "$stamp" bin/BUILD.txt
cat bin/BUILD.txt
