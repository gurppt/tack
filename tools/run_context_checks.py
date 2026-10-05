#!/usr/bin/env python3
"""Native contextual commands and low-resolution UI on an owned X11 display."""
import argparse
import json
import os
import struct
from pathlib import Path
import time
from PIL import Image
from run_local_production import Session, wait
from run_native_image_checks import command
from run_image_interaction import digest
from x11_drop import drop


def first_image_record(board):
    # Compare committed object authority, excluding container blob offsets/CRCs.
    data = board.read_bytes()
    sources, assets, _ = struct.unpack_from('<III', data, 48)
    offset = 96
    for _ in range(sources):
        length = struct.unpack_from('<I', data, offset)[0]
        offset += 4 + length
    offset += assets * 42
    assert struct.unpack_from('<HH', data, offset) == (1, 1)
    return data[offset:offset + 4 + 16 + 16 + 83]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    cfg = parser.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        parser.error('owned isolated X11 display required')
    root = cfg.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = cfg.binary.resolve()
    rows = []
    for size in ('800x600', '1024x768', '1600x900'):
        work = root / size
        work.mkdir()
        board = work / 'board.tack'
        image = work / 'image.png'
        Image.new('RGB', (220, 140), (190, 45, 30)).save(image)
        env = dict(os.environ, TACK_TEST_WINDOW_SIZE=size,
                   TACK_PROFILE_DIR=str(work / 'profile'))
        session = Session(binary, work, 'context', ['new', board], env)
        checks = []
        def record(name, condition):
            checks.append({'name': name, 'passed': bool(condition)})
            (work / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
            if not condition:
                raise AssertionError(size + ': ' + name)
        def pointer(x, y):
            command('xdotool', 'mousemove', '--window', session.window, str(x), str(y))
            time.sleep(.08)
        try:
            width, height = map(int, size.split('x'))
            # Blank-canvas menus at every corner; keyboard submenu and dismiss.
            for index, (x, y) in enumerate(((2, 24), (width-2, 24), (2, height-2), (width-2, height-2))):
                pointer(x, y)
                command('xdotool', 'click', '3')
                time.sleep(.15)
                session.shot(f'canvas-corner-{index}')
                session.key('Escape')
            session.key('F10', 'Down', 'Right')
            session.shot('file-submenu')
            session.key('Escape')
            pointer(180, 160)
            drop(session.window, [image])
            wait(lambda: 'modified' in session.title() and 'Import ' not in session.title(), 'import complete')
            imported = session.save(board)
            record('native import and save', len(imported['objects']) == 1)
            time.sleep(.5)
            session.shot('imported')
            capture = Image.open(work / 'context-imported.png').convert('RGB')
            points = [(x, y) for y in range(height) for x in range(width)
                      if (lambda c: c[0] > 145 and c[1] < 90 and c[2] < 80)(capture.getpixel((x, y)))]
            record('imported image has visible pixels', bool(points))
            px = (min(x for x, _ in points) + max(x for x, _ in points)) // 2
            py = (min(y for _, y in points) + max(y for _, y in points)) // 2
            pointer(px, py)
            command('xdotool', 'click', '3')
            session.shot('image-menu')
            # Flip horizontal through the contextual command, then common history.
            session.key('Down', 'Down', 'Return')
            flipped_bytes = None
            session.save(board)
            flipped_bytes = first_image_record(board)
            session.key('ctrl+z')
            session.save(board)
            original_bytes = first_image_record(board)
            record('context command commits to shared Undo', flipped_bytes != original_bytes)
            session.key('ctrl+shift+z')
            session.save(board)
            record('shared Redo restores contextual command', first_image_record(board) == flipped_bytes)
            session.key('ctrl+z')
            session.save(board)
            # A long drag must be a single history entry.
            pointer(px, py)
            command('xdotool', 'mousedown', '1')
            for step in range(1, 9):
                pointer(px + step*8, py + step*4)
            command('xdotool', 'mouseup', '1')
            moved = session.save(board)
            record('long drag commits geometry', moved != imported)
            session.key('ctrl+z')
            record('one Undo restores whole gesture', session.save(board) == imported)
            session.key('F10', 'Down', 'Down', 'Right')
            for _ in range(6):
                session.key('Down')
            session.key('Return')
            session.shot('keymap')
            session.key('Escape', 'Escape')
            # Preferences have a ten-row panel; test both ordinary integer scales.
            for scale in (1, 2):
                session.key('ctrl+comma')
                if scale == 2:
                    session.key('Down', 'Down', 'Down', 'Return', 'Return')
                session.shot(f'preferences-{scale}x')
                session.key('Escape')
                pointer(width-2, height-2)
                command('xdotool', 'click', '3')
                session.shot(f'corner-{scale}x')
                session.key('Escape')
            session.save(board)
            session.close()
            report = json.loads(session.report.read_text())
            record('actual native viewport matches requested size', report['window_size'] == [width, height])
            record('explicit 2x UI scale applied', report['local']['ui_scale'] == 2.)
            record('no supply errors', report['errors'] == 0)
            rows.append({'size': size, 'checks': checks,
                         'report_sha256': digest(session.report),
                         'screenshots': {p.name: digest(p) for p in work.glob('*.png') if p != image}})
            (root / 'summary.json').write_text(json.dumps({
                'binary_sha256': digest(binary), 'harness_sha256': digest(__file__),
                'scope': 'native NVIDIA/X11 automation and captures; no human comfort or Windows desktop claim',
                'rows': rows}, indent=2) + '\n')
            print(size, len(checks), 'checks', flush=True)
        finally:
            session.kill()


if __name__ == '__main__':
    main()
