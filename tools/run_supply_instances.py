#!/usr/bin/env python3
"""Two owned heavy native windows, alternating input, independent save/recovery."""
import argparse
import json
import os
from pathlib import Path
import shutil
import time
from run_local_production import Session, wait
from run_idle import observe
from run_image_interaction import digest
from run_multi_instance_resources import graphics_memory
from run_native_image_checks import command


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--fixtures', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--small', action='store_true')
    a = p.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        p.error('isolated X11 required')
    root = a.output.resolve(); root.mkdir(parents=True, exist_ok=False)
    sessions = []; rows = []; binary = a.binary.resolve()
    env = dict(os.environ, TACK_PROFILE_DIR=str(root/'profile'))
    try:
        for label, fixture in [('large', 'images-5k'), ('heavy', 'small' if a.small else 'images-1k')]:
            board = root/f'{label}.tack'; shutil.copyfile(a.fixtures/f'{fixture}.tack', board)
            sessions.append(Session(binary, root, label, ['open', board], env))
        time.sleep(3)
        # Alternate resize, zoom reversal and pan; other window remains idle.
        baseline = [observe(s.process.pid) for s in sessions]
        for s in sessions:
            command('xdotool', 'windowraise', s.window)
            s.focus()
            command('xdotool', 'mousemove', '--window', s.window, '700', '420')
            for _ in range(6):
                command('xdotool', 'click', '--repeat', '4', '--delay', '20', '4')
                command('xdotool', 'click', '--repeat', '4', '--delay', '20', '5')
            command('xdotool', 'mousedown', '2')
            command('xdotool', 'mousemove', '--window', s.window, '600', '350')
            command('xdotool', 'mouseup', '2')
            command('xdotool', 'windowsize', s.window, '1100', '800')
            s.key('r')
            command('xdotool', 'mousemove', '--window', s.window, '600', '420')
            time.sleep(.1)
            command('xdotool', 'mousedown', '1')
            for i in range(1,7):
                command('xdotool', 'mousemove', '--window', s.window, str(600+i*10), str(420+i*6))
                time.sleep(.05)
            command('xdotool', 'mouseup', '1')
            wait(lambda: 'modified' in s.title(), 'committed edit in each heavy window')
            s.key('v')
        # Let both recovery deadlines publish before normal Save.
        time.sleep(7)
        recovery = []
        for s in sessions:
            files = [f for f in root.rglob('*') if f.is_file() and 'recovery' in str(f.relative_to(root))]
            recovery.append({'name': s.name, 'dirty_title': s.title(),
                             'all_recovery_files_bytes': sum(f.stat().st_size for f in files)})
            s.save(root/f'{s.name}.tack')
        time.sleep(2)
        before = [observe(s.process.pid) for s in sessions]
        time.sleep(5)
        after = [observe(s.process.pid) for s in sessions]
        memory = graphics_memory({str(s.process.pid) for s in sessions})
        for s, b, e, idle0, idle1 in zip(sessions, baseline, after, before, after):
            io = {k: idle1['io'][k]-idle0['io'][k] for k in idle0['io']}
            ticks = sum(t['ticks']-idle0['tasks'].get(tid,t)['ticks'] for tid,t in idle1['tasks'].items())
            s.close()
            report = json.loads(s.report.read_text())
            rows.append({'name': s.name, 'rss_bytes': e['rss_bytes'], 'initial_rss_bytes': b['rss_bytes'],
                         'threads': len(e['tasks']), 'idle_ticks_5s': ticks, 'idle_io_delta': io,
                         'main_thread_ticks': idle1['tasks'][str(s.process.pid)]['ticks']-idle0['tasks'][str(s.process.pid)]['ticks'],
                         'native_report_sha256': digest(s.report), 'recoveries': report['local']['recoveries'], 'recovery_bytes': report['local']['recovery_bytes'],
                         'save_completed': report['editing']['save_completed'], 'dirty': report['editing']['dirty']})
        result = {'binary_sha256': digest(binary), 'rows': rows, 'recovery_observations': recovery,
                  'graphics_memory': memory, 'whole_process_rss_bytes': sum(r['rss_bytes'] for r in rows),
                  'scope': 'two processes, shared immutable source files, separate authority/worker/cache; alternating input; 5s final idle; total OS memory not attributed'}
        (root/'summary.json').write_text(json.dumps(result, indent=2)+'\n')
        if any(r['dirty'] or not r['save_completed'] or not r['recoveries'] or r['main_thread_ticks'] or any(r['idle_io_delta'].values()) for r in rows):
            raise AssertionError('multi-instance save/idle gate')
    finally:
        for s in sessions:
            s.kill()


if __name__ == '__main__':
    main()
