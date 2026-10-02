#!/usr/bin/env bash
# Optional host helper for sessions that cannot access network or GPU devices.
set -uo pipefail
cd -- "$(dirname -- "$0")/.."
export CARGO_HOME=/tmp/tack-cargo
if [ "$(/tmp/tack-tools/bin/cargo-deny --version 2>/dev/null)" != "cargo-deny 0.20.2" ]; then
    if ! cargo install cargo-deny --version 0.20.2 --locked --root /tmp/tack-tools; then
        exit 1
    fi
fi
/tmp/tack-tools/bin/cargo-deny check --hide-inclusion-graph > /tmp/tack-deny.log 2>&1
deny_status=$?
cat /tmp/tack-deny.log
if ! cargo build --release --locked; then
    exit 1
fi
python3 tools/run_benchmarks.py
bench_status=$?
if [ "$deny_status" -ne 0 ]; then exit "$deny_status"; fi
exit "$bench_status"
