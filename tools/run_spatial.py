#!/usr/bin/env python3
"""Phase1D generated metadata and serialized native GPU scenarios, frozen evidence."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import time
import zipfile
from run_image_interaction import distribution, digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/tack-app'))
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--seconds', type=float, default=12)
    args = parser.parse_args()
    if not 3 <= args.seconds <= 120:
        parser.error('duration 3..120 seconds')
    root = args.output.resolve(); root.mkdir(parents=True, exist_ok=False)
    binary = root / 'tack-app'; shutil.copy2(args.binary, binary)
    project = Path(__file__).resolve().parent.parent
    paths = [project / n for n in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml')]
    paths += [p for p in (project / 'crates').rglob('*') if p.suffix in ('.rs', '.wgsl', '.toml')]
    paths += list((project / 'tools').glob('*.py')) + list((project / '.cargo').glob('*.toml'))
    paths += [p for p in (project / 'assets/pixel-font').glob('*') if p.is_file()]
    with zipfile.ZipFile(root / 'source-snapshot.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
        for p in sorted(paths): archive.write(p, p.relative_to(project))
    metadata = root / 'metadata'
    with (root / 'metadata.log').open('w') as log:
        subprocess.run([str(binary), 'spatial-scale', str(metadata)], stdout=log, stderr=subprocess.STDOUT, check=True, timeout=30)
    cases = [(f'snap-{n}', metadata / f'snap-{n}.tack', 'snap') for n in (1000, 5000, 10000)]
    cases += [(mode, metadata / 'snap-1000.tack', mode) for mode in ('grid-hidden', 'grid-visible')]
    cases += [(f'frames-{n}', metadata / f'frames-{n}.tack', 'grid-hidden') for n in (10, 100, 1000)]
    runs = []
    for name, board, scenario in cases:
        report = root / f'{name}.json'
        command = [str(binary), 'open', str(board), '--seconds', str(args.seconds), '--interaction', scenario, '--output', str(report)]
        started = time.monotonic(); peak = 0
        with (root / f'{name}.log').open('w') as log:
            process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
            try:
                while process.poll() is None:
                    if time.monotonic() - started > args.seconds + 35:
                        raise RuntimeError('bounded spatial benchmark deadline exceeded')
                    try:
                        for line in Path(f'/proc/{process.pid}/status').read_text().splitlines():
                            if line.startswith('VmHWM:'): peak = max(peak, int(line.split()[1]) * 1024)
                    except (FileNotFoundError, ProcessLookupError): pass
                    time.sleep(.02)
            finally:
                if process.poll() is None:
                    process.terminate()
                    try: process.wait(timeout=2)
                    except subprocess.TimeoutExpired: process.kill(); process.wait()
        if process.returncode: raise RuntimeError(f'{name}: child returned {process.returncode}')
        data = json.loads(report.read_text()); frames = data['frames']; interaction = data['interaction']
        if data['source_bytes_before_detail'] != 0 or not interaction['invariants'] or data['peak_pending'] > 16:
            raise AssertionError('spatial source/history/queue gate')
        if scenario == 'snap' and interaction['commits'] < 1: raise AssertionError('no snapped commit/undo cycle')
        if len(frames) < 100: raise AssertionError('insufficient native samples')
        runs.append({'name': name, 'input': distribution(interaction['input_ms']), 'query': distribution(interaction['snap_query_ms']),
                     'cpu': distribution([f['cpu_ms'] for f in frames]), 'gpu': distribution([f['pass_ms'] for f in data['gpu_samples']]),
                     'callback': distribution([f['callback_ms'] for f in frames]), 'present': distribution([f['present_ms'] for f in frames]),
                     'acquire': distribution([f['acquire_ms'] for f in frames]), 'rss_peak_bytes': peak, 'redraws': data['redraw_count'],
                     'waits': data['wait_count'], 'commits': interaction['commits'], 'source_bytes': data['source_bytes_before_detail'],
                     'report_sha256': digest(report), 'board_sha256': digest(board), 'command': command})
        print(name, 'input p99', runs[-1]['input']['p99_ms'], 'query p99', runs[-1]['query']['p99_ms'], flush=True)
    summary = {'binary_sha256': digest(binary), 'source_snapshot_sha256': digest(root / 'source-snapshot.zip'), 'harness_sha256': digest(__file__),
               'percentile': 'sorted floor(p*(n-1)); max retained', 'timing': 'input includes begin/commit/undo, query is inside input; GPU canvas pass; callback includes input/acquire/present',
               'allocations': 'not instrumented; no candidate vector/index or source I/O in snapping', 'native': runs,
               'metadata': json.loads((metadata / 'metadata.json').read_text())}
    (root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')


if __name__ == '__main__': main()
