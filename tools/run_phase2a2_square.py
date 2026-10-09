#!/usr/bin/env python3
"""Serial native 50k-square navigation and restart on an owned X11 display.

All sampling/timers are external. No application test camera or cache bypass.
Real input is tracked mathematically and checked against emitted frame poses.
"""
import argparse
import json
import math
import os
from pathlib import Path
import time

from run_idle import observe
from run_image_interaction import digest
from run_local_production import Session
from run_native_image_checks import command


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--board', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--potato', action='store_true')
    args = parser.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        parser.error('explicitly owned isolated display required')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ, TACK_PROFILE_DIR=str(root/'profile'),
               TACK_TEST_WINDOW_SIZE='800x600', TACK_NATIVE_NO_WM='1',
               WINIT_X11_SCALE_FACTOR='1')
    flags = ['--lod-debug'] + (['--potato'] if args.potato else [])
    samples, stages, sessions = [], [], []
    s = None
    center, zoom = [750., 750.], .4

    def launch(name):
        nonlocal s, center, zoom
        s = Session(args.binary.resolve(), root, name, ['open', args.board.resolve(), *flags], env)
        center, zoom = [750., 750.], .4
        command('xdotool', 'mousemove', '--window', s.window, '400', '300')

    def wheel(count):
        nonlocal zoom
        command('xdotool', 'mousemove', '--window', s.window, '400', '300')
        command('xdotool', 'click', '--repeat', str(abs(count)), '--delay', '0',
                '4' if count > 0 else '5')
        for _ in range(abs(count)):
            zoom = min(64., max(.001, zoom*math.exp(.3 if count > 0 else -.3)))

    def drag(dx, dy):
        command('xdotool', 'mousemove', '--window', s.window, '400', '300', 'mousedown', '2',
                'mousemove', '--window', s.window, str(400+dx), str(300+dy), 'mouseup', '2')
        center[0] -= dx/zoom
        center[1] -= dy/zoom
        command('xdotool', 'mousemove', '--window', s.window, '400', '300')

    def move_center(target):
        # Bounded on-window drags performed at overview zoom. Never send huge
        # cursor coordinates or interact with another client's window.
        for _ in range(4):
            delta = [max(-250, min(250, round((center[i]-target[i])*zoom))) for i in range(2)]
            if delta == [0, 0]:
                break
            drag(*delta)

    def sample():
        value = observe(s.process.pid)
        status = Path(f'/proc/{s.process.pid}/status').read_text().splitlines()
        children = []
        for task in Path(f'/proc/{s.process.pid}/task').iterdir():
            try:
                ids = (task/'children').read_text().split()
            except FileNotFoundError:
                continue
            for pid in ids:
                try:
                    fields = dict(line.split(':', 1) for line in Path(f'/proc/{pid}/status').read_text().splitlines() if ':' in line)
                    if 'jpeg' in fields.get('Name', ''):
                        children.append({'pid': int(pid), 'hwm_bytes': int(fields['VmHWM'].split()[0])*1024})
                except (FileNotFoundError, KeyError):
                    pass
        item = {'session': s.name, 'elapsed_ms': (time.monotonic()-s.started)*1000,
                'rss_bytes': value['rss_bytes'], 'hwm_bytes': next(int(line.split()[1])*1024 for line in status if line.startswith('VmHWM:')),
                'ticks': sum(task['ticks'] for task in value['tasks'].values()),
                'io': value['io'], 'children': children}
        samples.append(item)
        return item

    def settle(name, capture=False):
        begin = time.monotonic()
        old = sample()
        quiet = None
        while time.monotonic()-begin < 25:
            time.sleep(.05)
            current = sample()
            calm = current['io']['rchar'] == old['io']['rchar'] and current['io']['wchar'] == old['io']['wchar'] and current['ticks']-old['ticks'] <= 1
            if calm:
                if quiet is None:
                    quiet = time.monotonic()
                elif time.monotonic()-quiet >= 1.5:
                    break
            else:
                quiet = None
            old = current
        else:
            raise AssertionError('native supply failed to settle: '+name)
        stages.append({'session': s.name, 'name': name, 'elapsed_ms': current['elapsed_ms'],
                       'begin_ms': (begin-s.started)*1000, 'wait_seconds': time.monotonic()-begin,
                       'expected_camera': center.copy(), 'expected_zoom': zoom})
        if capture:
            s.shot(name)

    def close():
        s.close()
        sessions.append(json.loads(s.report.read_text()))

    try:
        launch('cold')
        wheel(-2)
        settle('A-fit', True)
        wheel(19)
        settle('B-deep', True)
        for _ in range(3):
            drag(-200, 0)
        settle('C-horizontal')
        for _ in range(3):
            drag(0, -160)
        settle('C-vertical', True)
        for _ in range(3):
            drag(0, 160)
        for _ in range(3):
            drag(200, 0)
        settle('E-exact-return', True)
        wheel(-19)
        settle('F-sharp-out', True)
        # Visit two corners separated by about46,000 source pixels. The small
        # overview remains drawable throughout; detail supply stays asynchronous.
        move_center([60., 60.])
        wheel(19)
        settle('D-first-extreme', True)
        wheel(-19)
        move_center([1440., 1440.])
        wheel(19)
        settle('D-opposite-extreme', True)
        wheel(-19)
        move_center([750., 750.])
        settle('F-final-fit')
        before = sample()
        time.sleep(2)
        after = sample()
        idle = {'ticks': after['ticks']-before['ticks'],
                'io': {key: after['io'][key]-before['io'][key] for key in before['io']},
                'begin_ms': before['elapsed_ms'], 'end_ms': after['elapsed_ms']}
        close()
        # Reopen identical document and profile. The *last* corner remains in
        # potato's15slots; the center may legitimately have been LRU-evicted.
        launch('reopen')
        wheel(-2)
        # Repeat the clamp/reversal once to reproduce the original fit zoom
        # exactly. Overview preparation below includes these transient demands.
        wheel(19)
        wheel(-19)
        move_center([1440., 1440.])
        settle('E-reopen-overview')
        wheel(19)
        settle('E-reopen-hot', True)
        close()
    finally:
        if s is not None and s.process.poll() is None:
            s.kill()
    (root/'stages.json').write_text(json.dumps(stages, indent=2)+'\n')
    receipts = []
    for stage in stages:
        report = sessions[0 if stage['session'] == 'cold' else 1]
        previous = [frame for frame in report['frames'] if frame['elapsed_ms'] <= stage['elapsed_ms']]
        frame = previous[-1]
        if abs(frame['zoom']-stage['expected_zoom']) > .0001 or any(abs(frame['camera'][i]-stage['expected_camera'][i]) > .05 for i in range(2)):
            raise AssertionError('real input pose differs from tracked camera: '+json.dumps(dict(stage=stage, frame=frame)))
        if frame['supply']['pending'] != 0 or frame['tiles_ready'] != frame['tiles_requested']:
            raise AssertionError('incomplete settled detail: '+json.dumps(frame))
        if stage['name'].startswith(('A-', 'F-', 'E-reopen-overview')) and frame['tiles_requested']:
            raise AssertionError('overview retained HD demand')
        receipts.append(dict(stage, frame=frame))
    by_stage = {stage['name']: stage for stage in receipts}
    warm_before = by_stage['C-vertical']['frame']['supply']
    warm_after = by_stage['E-exact-return']['frame']['supply']
    hot_before = by_stage['E-reopen-overview']['frame']['supply']
    hot_after = by_stage['E-reopen-hot']['frame']['supply']
    reuse = {'same_session': {field: warm_after[field]-warm_before[field] for field in ('region_jobs', 'source_bytes', 'tile_cache_hits')},
             'reopen': {field: hot_after[field]-hot_before[field] for field in ('region_jobs', 'source_bytes', 'tile_cache_hits')}}
    if reuse['same_session']['region_jobs'] or reuse['same_session']['source_bytes']:
        raise AssertionError('exact same-session return performed codec work')
    if reuse['reopen']['region_jobs'] or reuse['reopen']['source_bytes']:
        raise AssertionError('hot reopen performed codec/source work: '+json.dumps(reuse))
    hot_frame = by_stage['E-reopen-hot']['frame']
    if reuse['reopen']['tile_cache_hits'] < hot_frame['tiles_ready']:
        raise AssertionError('reopen failed to reuse all previously visited final tile keys: '+json.dumps(reuse))
    summary = {'binary_sha256' : digest(args.binary), 'board_sha256': digest(args.board),
               'potato': args.potato, 'stages': receipts, 'reuse': reuse, 'idle': idle,
               'process_hwm_peak': max(sample['hwm_bytes'] for sample in samples),
               'child_sampled_hwm_peak': max((child['hwm_bytes'] for sample in samples for child in sample['children']), default=0),
               'cpu_payload_peak': max(report['cpu_payload_peak'] for report in sessions),
               'gpu_payload_peak': max(frame['gpu_bytes'] for report in sessions for frame in report['frames']),
               'scope': 'RTX2060/Linux800x600; external50ms sampling; modern host constrained budgets, no legacy-CPU claim'}
    (root/'samples.json').write_text(json.dumps(samples, indent=2)+'\n')
    (root/'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
    print(json.dumps({key: value for key, value in summary.items() if key != 'stages'}, indent=2))


if __name__ == '__main__':
    main()
