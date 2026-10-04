#!/usr/bin/env python3
"""Phase1C native production gestures, bounded child lifetime and frozen evidence."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import time
import zipfile

SCENARIOS = ('drag', 'resize', 'rotate', 'crop', 'multi10', 'multi100', 'cancel')


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def distribution(values):
    values = sorted(values)
    def at(p):
        return values[int(p * (len(values) - 1))] if values else None
    return {'samples': len(values), 'p50_ms': at(.5), 'p99_ms': at(.99), 'max_ms': max(values, default=0)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=Path('target/release/tack-app'))
    parser.add_argument('--board', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--seconds', type=float, default=12)
    parser.add_argument('--scenarios', nargs='+', choices=SCENARIOS, default=SCENARIOS)
    args = parser.parse_args()
    if not 3 <= args.seconds <= 120:
        parser.error('duration must be 3..120 seconds')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = root / 'tack-app'
    shutil.copy2(args.binary, binary)
    # The driver edits only RAM, undoes each completed cycle, and never issues Save.
    board = args.board.resolve()
    initial = digest(board)
    project = Path(__file__).resolve().parent.parent
    sources = [project / p for p in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml')]
    sources += [p for p in (project / 'crates').rglob('*') if p.suffix in ('.rs', '.wgsl', '.toml')]
    sources += list((project / 'tools').glob('*.py')) + list((project / '.cargo').glob('*.toml'))
    sources += [p for p in (project / "assets/pixel-font").glob("*") if p.is_file()]
    sources += [p for p in (project / "assets/note-font").glob("*") if p.is_file()]
    with zipfile.ZipFile(root / 'source-snapshot.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(sources):
            archive.write(path, path.relative_to(project))
    provenance = {'binary_sha256': digest(binary), 'source_snapshot_sha256': digest(root / 'source-snapshot.zip'),
                  'harness_sha256': digest(__file__), 'board_sha256': initial, 'board_path': str(board),
                  'platform': platform.platform(), 'display': os.environ.get('DISPLAY'),
                  'percentile': 'sorted floor(p*(n-1))', 'rss': '/proc VmHWM sampled every 20ms',
                  'allocations': 'not instrumented; preview storage reused, no event queue',
                  'cpu_note': 'input_ms includes begin/commit/undo; render cpu_ms excludes driver, callback includes driver'}
    runs = []
    for scenario in args.scenarios:
        report = root / f'{scenario}.json'
        command = [str(binary), 'open', str(board), '--seconds', str(args.seconds), '--interaction', scenario, '--output', str(report)]
        started = time.monotonic()
        peak_rss = 0
        with (root / f'{scenario}.log').open('w') as log:
            process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
            try:
                while process.poll() is None:
                    if time.monotonic() - started > args.seconds + 35:
                        raise RuntimeError('native benchmark exceeded bounded shutdown deadline')
                    try:
                        for line in Path(f'/proc/{process.pid}/status').read_text().splitlines():
                            if line.startswith('VmHWM:'):
                                peak_rss = max(peak_rss, int(line.split()[1])*1024)
                    except (FileNotFoundError, ProcessLookupError):
                        pass
                    time.sleep(.02)
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=2)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
        if process.returncode:
            raise RuntimeError(f'{scenario}: child returned {process.returncode}; see retained log')
        data = json.loads(report.read_text())
        interaction = data['interaction']
        if not interaction['invariants'] or not (interaction['commits'] or interaction['cancels']):
            raise AssertionError('no complete verified interaction cycle')
        frames = data['frames']
        bounds = all(f['uploads'] <= 8 and f['upload_bytes'] <= 16*1024**2 and f['gpu_bytes'] <= 128*1024**2 and f['in_flight'] <= 3 for f in frames)
        if not bounds or data['peak_pending'] > 16 or data['source_bytes_before_detail'] != 0:
            raise AssertionError('resource/source-I/O gate failed')
        runs.append({'name': scenario, 'command': command, 'report_sha256': digest(report), 'peak_rss_bytes': peak_rss,
                     'input': distribution(interaction['input_ms']), 'cpu': distribution([f['cpu_ms'] for f in frames]),
                     'callback': distribution([f['callback_ms'] for f in frames]), 'present': distribution([f['present_ms'] for f in frames]),
                     'acquire': distribution([f['acquire_ms'] for f in frames]), 'gpu': distribution([f['pass_ms'] for f in data['gpu_samples']]),
                     'commits': interaction['commits'], 'cancels': interaction['cancels'], 'invariants': True, 'bounds': bounds,
                     'source_bytes': data['source_bytes_before_detail'], 'peak_pending': data['peak_pending'],
                     'end_pending': data['navigation_end_pending'], 'overview_reused': data['overview_reused'],
                     'first_useful_ms': data['ordinary_view_80_percent_ms'], 'frames': len(frames)})
        if digest(board) != initial:
            raise AssertionError('benchmark mutated its input board')
        (root / 'summary.json').write_text(json.dumps({'provenance': provenance, 'runs': runs}, indent=2))
        print(scenario, json.dumps(runs[-1]), flush=True)


if __name__ == '__main__':
    main()
