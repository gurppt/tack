#!/usr/bin/env python3
"""Serial Phase1L/2A native local pay-for-play receipts on an owned X11 display."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time
from run_idle import observe
from run_local_production import Session
from run_image_interaction import digest, distribution


def sockets(pid):
    found = []
    for descriptor in Path(f'/proc/{pid}/fd').iterdir():
        try:
            target = os.readlink(descriptor)
        except FileNotFoundError:
            continue
        if target.startswith('socket:['):
            found.append(target[8:-1])
    tables = {}
    for kind in ('tcp', 'tcp6', 'udp', 'udp6', 'unix'):
        path = Path(f'/proc/{pid}/net/{kind}')
        rows = path.read_text().splitlines()[1:]
        for row in rows:
            fields = row.split()
            inode = fields[6] if kind == 'unix' else fields[9]
            if inode in found:
                tables[inode] = {'kind': kind, 'record': row.strip()}
    return list(tables.values())


def snapshot(pid):
    data = observe(pid)
    data['sockets'] = sockets(pid)
    return data


def delta(before, after, seconds):
    tasks = [{
        'name': row['name'],
        'ticks': row['ticks'] - before['tasks'][tid]['ticks'],
        'voluntary_switches': row['voluntary_ctxt_switches'] - before['tasks'][tid]['voluntary_ctxt_switches'],
    } for tid, row in after['tasks'].items() if tid in before['tasks']]
    ticks = sum(row['ticks'] for row in tasks)
    return {'seconds': seconds, 'rss_before': before['rss_bytes'], 'rss_after': after['rss_bytes'],
            'threads': len(after['tasks']), 'tasks': tasks, 'ticks': ticks,
            'cpu_percent_one_core': ticks / os.sysconf('SC_CLK_TCK') / seconds * 100,
            'socket_records': after['sockets'],
            'io_delta': {key: after['io'][key] - before['io'][key] for key in before['io']}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--current', type=Path, required=True)
    parser.add_argument('--board', type=Path, action='append', required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--baseline-label', default='baseline1l')
    parser.add_argument('--current-label', default='current2a')
    parser.add_argument('--repeats', type=int, choices=range(1,4), default=3)
    args = parser.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        parser.error('owned isolated X11 display required')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    data = {'harness_sha256': digest(__file__), 'runs': [],
            'scope': 'Warm/unspecified OS page cache; serial native800x600 samehost/display. Socket records distinguish OS X11 Unix sockets from Tack-created TCP/UDP. CPU /proc ticks, no perprocess VRAM attribution.'}
    for repeat in range(args.repeats):
        for board in args.board:
            for mode, binary in [(args.baseline_label, args.baseline), (args.current_label, args.current)]:
                name = f'{repeat}-{board.stem}-{mode}'
                runroot = root / name
                runroot.mkdir()
                env = dict(os.environ, TACK_PROFILE_DIR=str(runroot / 'profile'), TACK_NATIVE_NO_WM='1', TACK_TEST_WINDOW_SIZE='800x600')
                session = Session(binary.resolve(), runroot, name, ['open', board.resolve()], env)
                try:
                    time.sleep(6)
                    settle_start = time.monotonic()
                    previous = snapshot(session.process.pid)
                    quiet = None
                    while time.monotonic() - settle_start < 30:
                        time.sleep(.5)
                        current = snapshot(session.process.pid)
                        ticks = lambda sample: sum(row['ticks'] for row in sample['tasks'].values())
                        if ticks(current) - ticks(previous) <= 1 and current['io']['wchar'] == previous['io']['wchar']:
                            quiet = quiet or time.monotonic()
                            if time.monotonic() - quiet >= 1.5: break
                        else: quiet = None
                        previous = current
                    quiet_detected = quiet is not None and time.monotonic()-quiet >= 1.5
                    before = snapshot(session.process.pid)
                    started = time.monotonic()
                    time.sleep(3)
                    after = snapshot(session.process.pid)
                    seconds = time.monotonic() - started
                    session.close()
                    report = json.loads(session.report.read_text())
                    interval = delta(before, after, seconds)
                    if any(socket['kind'] != 'unix' for socket in interval['socket_records']):
                        raise AssertionError('ordinary local board opened IP socket')
                    if any('shared' in task['name'] or 'lan' in task['name'] for task in interval['tasks']):
                        raise AssertionError('ordinary local board started collaboration worker')
                    data['runs'].append({'name': name, 'mode': mode, 'binary_sha256': digest(binary), 'board_sha256': digest(board),
                        'quiet_detected':quiet_detected,'settle_seconds':time.monotonic()-settle_start-seconds,
                        'idle': interval, 'report': str(session.report.relative_to(root)), 'report_sha256': digest(session.report),
                        'native_startup_ms': report['native_startup_ms'], 'first_frame_ms': report['first_frame_ms'],
                        'first_recognizable_ms': report['first_recognizable_ms'], 'useful_ms': report['ordinary_view_80_percent_ms'],
                        'gpu_bytes_peak': max((frame['gpu_bytes'] for frame in report['frames']), default=0),
                        'callbacks': distribution([frame['callback_ms'] for frame in report['frames']]),
                        'wait_count': report['wait_count'], 'redraw_count': report['redraw_count'],
                        'idle_redraws': sum(started-session.started <= frame['elapsed_ms']/1000 <= started-session.started+seconds for frame in report['frames'])})
                    (root / 'summary.json').write_text(json.dumps(data, indent=2) + '\n')
                    print(name, interval['ticks'], 'ticks', interval['rss_after'], 'RSS', flush=True)
                finally:
                    session.kill()
    print(root / 'summary.json')


if __name__ == '__main__':
    main()
