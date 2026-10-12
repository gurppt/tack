#!/bin/sh
set -eu
cd -- "$(dirname -- "$0")"
exec ./tack-diagnostics export
