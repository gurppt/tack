#!/usr/bin/env python3
"""Native Phase1B vertical-slice evidence. Only generated benchmark corpus is used."""
import argparse
import hashlib
import json
import os
import platform
import zipfile
from pathlib import Path
import shutil
import subprocess
import time


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda: f.read(128 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=Path('target/release/tack-app'))
    parser.add_argument('--corpus', type=Path, default=Path('benchmark-data/mission0'))
    parser.add_argument('--output', type=Path, default=Path('benchmark-results/phase1b-product'))
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    binary = out / 'tack-app'
    shutil.copy2(args.binary, binary)
    original_manifest = args.corpus.resolve() / 'manifest.json'
    manifest = json.loads(original_manifest.read_text())
    corpus = out / 'corpus'
    corpus.mkdir()
    for row in manifest['objects']:
        original = (original_manifest.parent / row['path']).resolve()
        if not original.is_relative_to(original_manifest.parent):
            raise ValueError('benchmark source escapes corpus')
        target = corpus / row['path']
        target.parent.mkdir(parents=True, exist_ok=True)
        os.link(original, target)
    (corpus / 'manifest.json').write_text(json.dumps(manifest))
    source_root = Path(__file__).resolve().parent.parent
    source_paths = [source_root / name for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']]
    source_paths += [p for p in (source_root / 'crates').rglob('*') if p.suffix in ['.rs', '.wgsl', '.toml']]
    source_paths += list((source_root / 'tools').glob('*.py')) + list((source_root / '.cargo').glob('*.toml'))
    source_hash = hashlib.sha256()
    with zipfile.ZipFile(out / 'source-snapshot.zip', 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(source_paths):
            relative = str(path.relative_to(source_root))
            content = path.read_bytes()
            source_hash.update(relative.encode() + b'\0' + content + b'\0')
            archive.writestr(relative, content)
    inventory = []
    for row in manifest['objects']:
        path = corpus / row['path']
        actual = digest(path)
        if actual != row['source_sha256']:
            raise ValueError('benchmark source content differs from manifest')
        inventory.append({'path': row['path'], 'bytes': path.stat().st_size, 'sha256': actual})
    (out / 'source-inventory.json').write_text(json.dumps(inventory, indent=2))
    provenance = {'binary_sha256': digest(binary), 'source_sha256': source_hash.hexdigest(),
        'source_snapshot_sha256': digest(out / 'source-snapshot.zip'),
        'harness_sha256': digest(Path(__file__)), 'input_manifest_sha256': digest(original_manifest),
        'source_inventory_sha256': digest(out / 'source-inventory.json'), 'platform': platform.platform(),
        'percentile': 'sorted value at floor(p*(n-1)); frame coverage weighted by visible objects',
        'rss': 'sampled /proc VmHWM every 10 ms; peak may be missed for very short processes',
        'cache': 'fresh process RAM/VRAM; kernel page cache not flushed',
        'native_build': json.loads((source_root / 'target/native/libjpeg-turbo-3.2.0/native-build.json').read_text()),
        'native_library_sha256': digest(source_root / 'target/native/libjpeg-turbo-3.2.0/lib/libturbojpeg.a')}
    for name, command in {'rustc': ['rustc', '--version'], 'gpu': ['nvidia-smi', '--query-gpu=name,driver_version,memory.total', '--format=csv']}.items():
        provenance[name] = subprocess.run(command, capture_output=True, text=True, check=True).stdout
    results = []

    def run(name, arguments, report=True):
        destination = out / f'{name}.json'
        command = [str(binary), *map(str, arguments)]
        if report:
            command.extend(['--output', str(destination)])
        start = time.monotonic()
        peak_rss = 0
        with (out / f'{name}.log').open('w') as log:
            process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
            while process.poll() is None:
                try:
                    for line in Path(f'/proc/{process.pid}/status').read_text().splitlines():
                        if line.startswith('VmHWM:'):
                            peak_rss = max(peak_rss, int(line.split()[1]) * 1024)
                except (FileNotFoundError, ProcessLookupError):
                    pass
                time.sleep(0.01)
            if process.returncode:
                raise RuntimeError(f'{name} exited {process.returncode}; see log')
        result = {'name': name, 'command': command, 'wall_ms': (time.monotonic()-start)*1000,
                  'peak_rss_bytes': peak_rss, 'report_sha256': digest(destination)}
        data = json.loads(destination.read_text())
        result['summary'] = {k: v for k, v in data.items() if k not in ('frames', 'gpu_samples')}
        if 'frames' in data:
            frames = data['frames']
            def percentile(key):
                values = sorted(f[key] for f in frames)
                return values[int(0.99*(len(values)-1))] if values else None
            samples = sorted(s['pass_ms'] for s in data['gpu_samples'])
            result['frame_summary'] = {'frames': len(frames), 'p99_representative': len(frames) >= 100,
                'cpu_max_ms': max((f['cpu_ms'] for f in frames), default=0),
                'upload_max_ms': max((f['upload_cpu_ms'] for f in frames), default=0),
                'callback_max_ms': max((f['callback_ms'] for f in frames), default=0),
                'cpu_p99_ms': percentile('cpu_ms'),
                'upload_p99_ms': percentile('upload_cpu_ms'), 'callback_p99_ms': percentile('callback_ms'),
                'gpu_pass_p99_ms': samples[int(0.99*(len(samples)-1))] if samples else None,
                'recognizable_fraction': sum(f['recognizable'] for f in frames)/max(1,sum(f['visible'] for f in frames)),
                'bounds_pass': all(f['uploads'] <= 8 and f['upload_bytes'] <= 16*1024*1024
                    and f['in_flight'] <= 3 and f['gpu_bytes'] <= 128*1024*1024 for f in frames)}
            if not result['frame_summary']['bounds_pass']:
                raise AssertionError('product GPU bound exceeded')
        results.append(result)
        print(name, json.dumps(result['summary']), flush=True)
        (out / 'summary.json').write_text(json.dumps({'provenance': provenance, 'runs': results}, indent=2))
        return data

    board = out / 'linked.tack'
    run('create-linked-1k', ['create', board, '--linked', '--manifest', corpus / 'manifest.json'])
    run('reopen-prepared', ['open', board, '--seconds', '6'])
    run('reopen-warm', ['open', board, '--seconds', '6'])
    run('reopen-tour', ['open', board, '--seconds', '12', '--board-tour'])
    # Hide only this harness's links, never the input corpus or private sources.
    hidden = []
    try:
        for row in manifest['objects'][:3]:
            path = corpus / row['path']
            temporary = path.with_suffix(path.suffix + '.hidden')
            path.rename(temporary)
            hidden.append((temporary, path))
        run('reopen-missing', ['open', board, '--seconds', '6'])
    finally:
        for temporary, path in hidden:
            temporary.rename(path)
    corrupt = out / 'corrupt-overviews.tack'
    shutil.copy2(board, corrupt)
    with corrupt.open('r+b') as f:
        header = f.read(80)
        auth_len = int.from_bytes(header[16:24], 'little')
        count = int.from_bytes(header[64:68], 'little')
        assert count == 1000
        f.seek(80 + auth_len)
        directory = f.read(count*64)
        for index in [0, 499, 999]:
            offset = int.from_bytes(directory[index*64+44:index*64+52], 'little')
            f.seek(offset)
            f.write(b'corrupt overview')
    run('repair-three', ['repair', corrupt, out / 'repaired.tack', out / 'repair-three.json'], report=False)
    run('reopen-repaired', ['open', out / 'repaired.tack', '--seconds', '6'])
    # Bounded generated real image, embedded survival after external deletion.
    from PIL import Image
    fixture = out / 'embedded-fixture.png'
    Image.new('RGB', (96, 64), (160, 50, 25)).save(fixture)
    run('create-embedded', ['create', out / 'embedded.tack', '--embedded', fixture])
    fixture.unlink()
    run('reopen-embedded-after-deletion', ['open', out / 'embedded.tack', '--seconds', '3'])
    run('query-scale', ['query-scale', out / 'query-scale.json'], report=False)


if __name__ == '__main__':
    main()
