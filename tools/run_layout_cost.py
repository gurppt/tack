#!/usr/bin/env python3
"""Measure one-shot layout CPU and allocator cost outside the native runtime."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--memusage', type=Path,
                        default=Path('/usr/lib/x86_64-linux-gnu/libmemusage.so'))
    cfg = parser.parse_args()
    root = cfg.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = cfg.binary.resolve()
    rows = []
    for n in (10, 100, 1000, 10000):
        runs = [json.loads(subprocess.check_output([str(binary), str(n), 'full'], text=True))
                for _ in range(7)]
        allocations = {}
        for mode in ('baseline', 'plan', 'full'):
            result = subprocess.run([str(binary), str(n), mode],
                                    env=dict(os.environ, LD_PRELOAD=str(cfg.memusage.resolve())),
                                    capture_output=True, text=True, check=True)
            (root / f'{n}-{mode}-allocator.log').write_text(result.stderr)
            (root / f'{n}-{mode}.json').write_text(result.stdout)
            text = re.sub(r'\x1b\[[0-9;]*m', '', result.stderr)
            match = re.search(r'heap total:\s*(\d+), heap peak:\s*(\d+), stack peak:\s*(\d+)', text)
            if not match:
                raise AssertionError(text)
            allocations[mode] = {'heap_total': int(match[1]), 'heap_peak': int(match[2]),
                                 'stack_peak': int(match[3])}
        rows.append({'n': n, 'uninstrumented_runs': runs, 'allocator': allocations,
                     'plan_extra_requested_bytes': allocations['plan']['heap_total'] - allocations['baseline']['heap_total'],
                     'plan_extra_peak_bytes': allocations['plan']['heap_peak'] - allocations['baseline']['heap_peak'],
                     'apply_extra_requested_bytes': allocations['full']['heap_total'] - allocations['plan']['heap_total']})
    (root / 'summary.json').write_text(json.dumps({
        'scope': '7 uninstrumented CPU samples per size; separate glibc memusage observations include identical fixture and visibility rebuild; allocator deltas include small reporting differences and are not exact Rust liveness; run serially without compiler/native GPU overlap',
        'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'harness_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'rows': rows}, indent=2) + '\n')
    print('layout CPU/allocator series complete', flush=True)


if __name__ == '__main__':
    main()
