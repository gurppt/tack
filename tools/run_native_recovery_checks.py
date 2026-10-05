#!/usr/bin/env python3
"""Owned native relink, failed Save, close cancellation and corrupt recovery checks."""
import argparse
import json
import os
from pathlib import Path
import struct
import subprocess
import time
from PIL import Image
from run_local_production import Session, wait
from run_native_image_checks import command
from run_native_annotation_checks import saved_objects
from run_image_interaction import digest
from run_idle import close_owned_window, observe
from x11_drop import drop


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('TACK_NATIVE_NO_WM') != '1' or os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        parser.error('isolated X11 required')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    env = dict(os.environ, TACK_PROFILE_DIR=str(root / 'profile'), GSK_RENDERER='cairo',
               GSETTINGS_BACKEND='memory', GIO_USE_VFS='local', XDG_DATA_HOME=str(root / 'xdg-data'), XDG_CONFIG_HOME=str(root / 'xdg-config'))
    sessions, checks = [], []
    def start(name, arguments):
        s = Session(binary, root, name, arguments, env)
        sessions.append(s)
        return s
    def record(name, ok):
        checks.append({'name': name, 'observed': bool(ok)})
        (root / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
        if not ok:
            raise AssertionError(name)
    def revision(board):
        return struct.unpack_from('<Q', board.read_bytes(), 118)[0]
    def note(s, text):
        s.key('t')
        command('xdotool', 'mousemove', '--window', s.window, '500', '240')
        command('xdotool', 'click', '1')
        s.text(text)
        s.key('ctrl+Return')
    board, a, b = root / 'shared.tack', root / 'a.png', root / 'b.png'
    Image.new('RGB', (240, 120), (180, 45, 25)).save(a)
    Image.new('RGB', (90, 240), (30, 110, 190)).save(b)
    hidden = root / 'a.png.hidden'
    try:
        s = start('linked', ['new', board])
        s.key('ctrl+comma', 'Down', 'Down', 'Return', 'Escape')
        drop(s.window, [a, a])
        wait(lambda: 'modified' in s.title() and 'Import ' not in s.title(), 'shared linked admission')
        original = s.save(board)
        record('shared native import uses one source/asset and two objects', struct.unpack_from('<III', board.read_bytes(), 48) == (1, 1, 2))
        s.close()
        a.rename(hidden)
        missing = start('missing', ['open', board])
        wait(lambda: '1 missing' in missing.title(), 'explicit missing state')
        missing.shot('last-known')
        im = Image.open(root / 'missing-last-known.png').convert('RGB')
        points = [(x, y) for y in range(im.height) for x in range(im.width) if (lambda c: c[0] > 145 and c[1] < 100 and c[2] < 80)(im.getpixel((x, y)))]
        record('missing shared source keeps last-known preview visible', bool(points))
        # First imported object, before the second column.
        command('xdotool', 'mousemove', '--window', missing.window,
                str(min(x for x, y in points) + 30), str(min(y for x, y in points) + 30))
        command('xdotool', 'click', '1')
        wait(lambda: '1 selected' in missing.title(), 'one missing image selected')
        missing.picker('ctrl+shift+r', b)
        wait(lambda: 'modified' in missing.title(), 'relink commit')
        relinked = missing.save(board)
        rev = revision(board)
        record('native shared relink preserves IDs/layout and advances revision', relinked == original and rev > 1)
        missing.key('ctrl+z')
        record('native relink undo restores source revision with exact geometry', missing.save(board) == original and revision(board) == 1)
        missing.key('ctrl+y')
        record('native relink redo retains fresh source revision', missing.save(board) == original and revision(board) == rev)
        missing.close()
        hidden.rename(a)
        failed = start('failed-save', ['open', board])
        # External authoritative replacement while this editor owns the old stamp.
        replacement = bytearray(board.read_bytes())
        replacement[12:16] = (0xffff).to_bytes(4, 'little')
        board.write_bytes(replacement)
        replacement_hash = digest(board)
        note(failed, 'Preserve me on save failure')
        failed.key('ctrl+s')
        wait(lambda: 'save failed' in failed.title(), 'external replacement failure visible')
        failed.shot('error')
        record('failed normal save refuses newer external bytes and exposes failure', digest(board) == replacement_hash and ('save error' in failed.title() or 'save failed' in failed.title()))
        failed.key('Escape')
        note(failed, 'Still editable after dismissing Save error')
        record('dismissed Save error keeps the real document editable', failed.process.poll() is None)
        close_owned_window(failed.window)
        time.sleep(.2)
        failed.key('Down', 'Return')
        failed.process.wait(timeout=8)
        failed.log.close()
        failed_report = json.loads(failed.report.read_text())
        record('failed Save retains committed dirty generation until explicit discard', failed_report['editing']['dirty'] and failed_report['editing']['generation'] >= 2 and not failed_report['load_failed'])
        record('explicit close discard still preserves external authoritative replacement', digest(board) == replacement_hash)
        recovery_board = root / 'recovery.tack'
        crash = start('crash', ['new', recovery_board])
        normal_hash = digest(recovery_board)
        note(crash, 'Retain after close cancel')
        close_owned_window(crash.window)
        time.sleep(.2)
        crash.key('Escape')
        record('dirty close Cancel keeps the window and work', crash.process.poll() is None and 'modified' in crash.title())
        control = root / '.recovery.tack.tack-recovery/state.meta'
        wait(control.exists, 'crash recovery published', seconds=12)
        crash.kill()
        control.write_bytes(b'bad')
        corrupt = start('corrupt-recovery', ['open', recovery_board])
        wait(lambda: 'Recovery invalid' in corrupt.title(), 'invalid recovery error')
        corrupt.shot('error')
        record('corrupt recovery leaves normal save exact', digest(recovery_board) == normal_hash)
        corrupt.key('Escape', 'Down', 'Return')
        wait(lambda: not control.exists(), 'explicit recovery discard')
        record('native corrupt recovery discard affects only owned artifacts', digest(recovery_board) == normal_hash)
        corrupt.close()
        owner = None
        for name, payload, dismiss in [('future', replacement, 'Return'), ('truncated', b'TACK', 'Escape'), ('locked', None, 'Return')]:
            if name == 'locked':
                owner = start('locked-owner', ['open', recovery_board])
            p = recovery_board if payload is None else root / f'{name}.tack'
            if payload is not None:
                p.write_bytes(payload)
            before = digest(p)
            rejected = start(name, ['open', p])
            rejected.shot('error')
            record(f'native {name} open exposes an error and preserves authority', digest(p) == before and len(saved_objects(recovery_board)['objects']) == 0)
            # Document shortcuts and external drops cannot turn the error-only
            # window into an unsaveable draft or arm its recovery timer.
            rejected.key('t')
            rejected.text('Unsavable text must be ignored')
            rejected.key('ctrl+s')
            drop(rejected.window, [b])
            if name == 'future':
                # Match the ordinary idle harness: settle native input and
                # lazy GPU pipeline/driver work before observing idle.
                time.sleep(3)
                first = observe(rejected.process.pid)
                time.sleep(6)
                last = observe(rejected.process.pid)
                ticks = sum(v['ticks'] - first['tasks'].get(k, v)['ticks'] for k, v in last['tasks'].items())
                main_first = first['tasks'][str(rejected.process.pid)]
                main_last = last['tasks'][str(rejected.process.pid)]
                main_ticks = main_last['ticks'] - main_first['ticks']
                main_wakes = main_last['voluntary_ctxt_switches'] - main_first['voluntary_ctxt_switches']
                (root / 'failed-open-idle.json').write_text(json.dumps({'first': first, 'last': last, 'cpu_ticks': ticks, 'main_cpu_ticks': main_ticks, 'main_voluntary_switches': main_wakes, 'scope': '6s after shortcuts/drop; driver CPU remains included in total, main-thread spin asserted separately'}, indent=2) + '\n')
                record('failed-open event loop sleeps without recovery spin or file writes', main_ticks == 0 and main_wakes == 0 and last['io']['write_bytes'] == first['io']['write_bytes'])
            rejected.key(dismiss)
            rejected.process.wait(timeout=8)
            rejected.log.close()
            report = json.loads(rejected.report.read_text())
            record(f'native {name} Dismiss closes error-only window without authoring or changing bytes', rejected.process.returncode == 0 and digest(p) == before and report['load_failed'] and not report['editing']['dirty'] and report['editing']['generation'] == 0 and report['local']['recoveries'] == 0)
        if owner is not None:
            owner.close()
    finally:
        if hidden.exists():
            hidden.rename(a)
        for s in sessions:
            if s.process.poll() is None:
                try:
                    (root / f'{s.name}-final-title.txt').write_text(s.title())
                    s.shot('failure')
                except Exception:
                    pass
            s.kill()
    (root / 'provenance.json').write_text(json.dumps({'binary_sha256': digest(binary), 'harness_sha256': digest(__file__), 'checks': len(checks), 'scope': 'isolated real X11 input and owned files; no Windows/power-loss claim'}, indent=2) + '\n')


if __name__ == '__main__':
    main()
