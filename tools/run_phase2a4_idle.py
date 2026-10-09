#!/usr/bin/env python3
"""Serial paired startup/idle plus hidden-toolbar/status cost on owned X11."""
import argparse
import copy
import json
import os
from pathlib import Path
import subprocess
import time
from run_local_production import Session
from run_phase2a_local_regression import snapshot, delta
from run_image_interaction import digest


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ['baseline', 'current', 'empty', 'dense', 'profile', 'output']:
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0') or os.environ.get('TACK_NATIVE_NO_WM') != '1':
        p.error('Owned isolated display required')
    root = a.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    with (root / 'paired.log').open('w') as log:
        subprocess.run(['python3', str(Path(__file__).with_name('run_phase2a_local_regression.py')),
            '--baseline', str(a.baseline.resolve()), '--current', str(a.current.resolve()),
            '--board', str(a.empty.resolve()), '--board', str(a.dense.resolve()),
            '--output', str(root / 'paired'), '--baseline-label', 'phase2a3',
            '--current-label', 'phase2a4-visible', '--repeats', '2'],
            stdout=log, stderr=subprocess.STDOUT, check=True)
    profile = json.loads(a.profile.read_text())
    extra = []
    for repeat in range(2):
        for name, status in [('hidden', True), ('bare', False)]:
            run = root / f'{name}-{repeat}'
            path = run / 'profile'
            path.mkdir(parents=True)
            settings = copy.deepcopy(profile)
            settings['toolbar']['placement'] = 'Hidden'
            settings['status_bar'] = status
            (path / 'preferences.json').write_text(json.dumps(settings))
            env = dict(os.environ, TACK_PROFILE_DIR=str(path), TACK_TEST_WINDOW_SIZE='800x600')
            s = Session(a.current.resolve(), run, name, ['open', a.empty.resolve()], env)
            try:
                time.sleep(6)
                before = snapshot(s.process.pid)
                started = time.monotonic()
                time.sleep(3)
                after = snapshot(s.process.pid)
                seconds = time.monotonic() - started
                s.close()
                report = json.loads(s.report.read_text())
                idle = delta(before, after, seconds)
                redraws = sum(started-s.started <= f['elapsed_ms']/1000 <= started-s.started+seconds
                              for f in report['frames'])
                if redraws or any(idle['io_delta'].values()) or any(r['kind'] != 'unix' for r in idle['socket_records']):
                    raise AssertionError('Hidden local UI did not remain quiet')
                extra.append(dict(mode=name, repeat=repeat, startup_ms=report['native_startup_ms'],
                    chrome=report['chrome'], idle=idle, idle_redraws=redraws,
                    report=str(s.report.relative_to(root)), report_sha256=digest(s.report)))
                (root / 'extra.json').write_text(json.dumps(extra, indent=2) + '\n')
            finally:
                s.kill()
    (root / 'receipt.json').write_text(json.dumps(dict(binary_sha256=digest(a.current),
        baseline_sha256=digest(a.baseline), harness_sha256=digest(__file__),
        paired='paired/summary.json', extra=extra), indent=2) + '\n')
    print(root / 'receipt.json')


if __name__ == '__main__':
    main()
