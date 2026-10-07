#!/usr/bin/env python3
"""Owned ordinary windows: empty/heavy menus and two-board settled idle."""
import argparse
import json
import os
from pathlib import Path
import time
from run_local_production import Session
from run_idle import observe
from run_image_interaction import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--current', type=Path, required=True)
    parser.add_argument('--heavy', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        parser.error('owned isolated X11 display required')
    root = args.output.resolve(); root.mkdir(parents=True, exist_ok=False)
    rows = []; heavy_hash = digest(args.heavy)
    for name, binary in (('baseline', args.baseline), ('current', args.current)):
        work = root / name; work.mkdir()
        env = dict(os.environ, TACK_NATIVE_NO_WM='1', TACK_TEST_WINDOW_SIZE='800x600',
                   TACK_PROFILE_DIR=str(work / 'profile'))
        sessions = []
        def start(label, arguments):
            session = Session(binary.resolve(), work, label, arguments, env)
            sessions.append(session)
            return session
        def interval(label, observed):
            time.sleep(3)
            begin = time.monotonic(); before = [observe(s.process.pid) for s in observed]
            time.sleep(2)
            after = [observe(s.process.pid) for s in observed]; end = time.monotonic()
            time.sleep(.2)
            for s, old, new in zip(observed, before, after):
                rows.append({'binary': name, 'case': label, 'session': s.name,
                             'begin_ms': (begin-s.started)*1000, 'end_ms': (end-s.started)*1000,
                             'rss_bytes': new['rss_bytes'], 'rss_delta': new['rss_bytes']-old['rss_bytes'],
                             'threads': len(new['tasks']), 'thread_delta': len(new['tasks'])-len(old['tasks']),
                             'ticks': sum(new['tasks'].get(t,v)['ticks']-v['ticks'] for t,v in old['tasks'].items()),
                             'io': {k: new['io'][k]-old['io'][k] for k in old['io']}})
        try:
            empty = start('empty', ['new', work / 'empty.tack'])
            interval('empty', [empty]); empty.key('F10'); interval('menu-open', [empty])
            empty.key('Escape'); interval('menu-closed', [empty]); empty.close()
            heavy = start('heavy', ['open', args.heavy.resolve()])
            interval('heavy', [heavy]); heavy.key('F10'); interval('heavy-menu-open', [heavy])
            heavy.key('Escape'); second = start('second', ['new', work / 'second.tack'])
            interval('two-boards', [heavy, second]); second.close(); heavy.close()
            for s in sessions:
                report = json.loads(s.report.read_text())
                for row in rows:
                    if row['binary'] == name and row['session'] == s.name:
                        row['idle_frames'] = sum(row['begin_ms']-100 <= f['elapsed_ms'] <= row['end_ms']+100 for f in report['frames'])
                        if row['idle_frames'] or any(row['io'].values()) or row['thread_delta']:
                            raise AssertionError(row)
                if s.name == 'heavy' and report['frames'][-1]['quality_resolved'] != report['frames'][-1]['visible']:
                    raise AssertionError('heavy board failed to converge')
        finally:
            for s in sessions: s.kill()
        receipt = {'baseline_sha256': digest(args.baseline), 'current_sha256': digest(args.current),
                   'heavy_sha256': heavy_hash, 'interval_seconds': 2, 'rows': rows}
        (root / 'receipt.json').write_text(json.dumps(receipt, indent=2)+'\n')
        print(name, 'seven idle observations passed', flush=True)
    if digest(args.heavy) != heavy_hash: raise AssertionError('board changed')


if __name__ == '__main__': main()
