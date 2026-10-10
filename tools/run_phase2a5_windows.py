#!/usr/bin/env python3
"""Windows CLI/JPEG execution under an owned disposable Wine prefix.

Not a physical Windows desktop test. Prefix scratch may use up to 2 GiB;
it is shut down and removed after checks, retaining only compact receipts.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

from PIL import Image
from run_image_interaction import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    package = args.package.resolve()
    prefix = root / 'wine-prefix'
    env = dict(os.environ, WINEPREFIX=str(prefix), WINEARCH='win64', WINEDEBUG='-all',
               WINEDLLOVERRIDES='mscoree,mshtml=d')
    checks = []

    def windows(path):
        return 'Z:' + str(path.resolve()).replace('/', '\\')

    def run(executable, *arguments, extra=None):
        result = subprocess.run(['/usr/bin/wine', str(executable), *arguments],
                                env=dict(env, **(extra or {})), capture_output=True,
                                text=True, timeout=90)
        (root / 'execution.log').open('a').write(result.stderr + '\n')
        if result.returncode:
            raise AssertionError((executable.name, result.returncode, result.stderr[-2000:]))
        return result.stdout

    def record(name, ok, detail=None):
        checks.append(dict(name=name, observed=bool(ok), detail=detail))
        (root / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
        if not ok:
            raise AssertionError(name)

    if shutil.disk_usage(root).free < 12 * 1024 ** 3:
        raise RuntimeError('Reserve at least 10 GiB before Wine verification')
    image = root / 'source.jpg'
    Image.new('RGB', (160, 100), (45, 130, 185)).save(image, quality=95)
    started = time.monotonic()
    try:
        run(package / 'tack.exe', '--help')
        board = root / 'windows-created.tack'
        created = json.loads(run(package / 'tack.exe', 'create', windows(board),
                                 '--embedded', windows(image)))
        inspected = json.loads(run(package / 'tack.exe', 'inspect', windows(board)))
        record('Windows executable creates/reopens JPEG board', board.exists()
               and inspected.get('objects') == 1, dict(created=created, inspected=inspected))
        outputs = []
        for mode, flag in [('scalar', 'JSIMD_FORCENONE'), ('sse2', 'JSIMD_FORCESSE2'),
                           ('automatic-simd', None)]:
            output = root / (mode + '.ppm')
            run(package / 'tack-jpeg-decoder.exe', '-outfile', windows(output), windows(image),
                extra={flag: '1'} if flag else {})
            decoded = Image.open(output)
            record('Windows JPEG decoder executes ' + mode, decoded.size == (160, 100),
                   dict(size=decoded.size, sha256=digest(output)))
            outputs.append(output.read_bytes())
        record('scalar/SSE2/automatic SIMD decoder output matches', len(set(outputs)) == 1)
        size = sum(p.stat().st_size for p in prefix.rglob('*') if p.is_file() and not p.is_symlink())
        record('owned Wine scratch stays within documented 2 GiB exception', size <= 2 * 1024 ** 3,
               dict(bytes=size))
        (root / 'receipt.json').write_text(json.dumps(dict(
            scope='Linux Wine Windows CLI/decoder execution; no Windows desktop/physical LAN claim',
            executable_sha256=digest(package / 'tack.exe'),
            decoder_sha256=digest(package / 'tack-jpeg-decoder.exe'),
            harness_sha256=digest(Path(__file__)), seconds=time.monotonic() - started,
            checks=checks), indent=2) + '\n')
    finally:
        subprocess.run(['/usr/bin/wineserver', '-k'], env=env, timeout=15, check=False)
        subprocess.run(['/usr/bin/wineserver', '-w'], env=env, timeout=15, check=False)
        if prefix.exists():
            shutil.rmtree(prefix)
    print(root / 'receipt.json')


if __name__ == '__main__':
    main()
