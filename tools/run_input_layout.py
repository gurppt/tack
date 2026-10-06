#!/usr/bin/env python3
"""Native US/AZERTY keyboard, tool completion and atomic arrangement evidence."""
import argparse
import json
import os
import time
from pathlib import Path
from PIL import Image
from run_local_production import Session, wait
from run_native_image_checks import command
from run_native_annotation_checks import saved_objects
from run_image_interaction import digest
from x11_drop import drop


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    args = p.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        p.error('requires owned isolated X11 display')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    rows = []
    for size in ('800x600', '1024x768'):
        for layout in ('us', 'fr'):
            command('setxkbmap', '-layout', layout)
            work = root / (size + '-' + layout)
            work.mkdir()
            board = work / 'board.tack'
            env = dict(os.environ, TACK_TEST_WINDOW_SIZE=size,
                       TACK_PROFILE_DIR=str(work / 'profile'), TACK_TRACE_INPUT='1')
            s = Session(binary, work, 'input', ['new', board], env)
            checks = []
            def record(name, ok):
                checks.append({'name': name, 'passed': bool(ok)})
                (work / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
                if not ok:
                    raise AssertionError(str(work) + ': ' + name)
            def pointer(x, y):
                command('xdotool', 'mousemove', '--window', s.window, x, y)
                time.sleep(.07)
            def drag(start, end):
                pointer(*start)
                command('xdotool', 'mousedown', '1')
                pointer(*end)
                command('xdotool', 'mouseup', '1')
                time.sleep(.15)
            def image_point():
                s.shot('image-location')
                capture = Image.open(work / 'input-image-location.png').convert('RGB')
                pts = [(x, y) for y in range(capture.height) for x in range(capture.width)
                       if (lambda c: c[0] > 145 and c[1] < 90 and c[2] < 90)(capture.getpixel((x, y)))]
                center = ((min(x for x, y in pts)+max(x for x, y in pts))//2,
                          (min(y for x, y in pts)+max(y for x, y in pts))//2)
                return min(pts, key=lambda p: (p[0]-center[0])**2 + (p[1]-center[1])**2)
            def arrange(row=0):
                pointer(*image_point())
                command('xdotool', 'click', '3')
                s.key('Down', 'Right')  # Multi-selection row zero: Arrange.
                if row:
                    s.key('Down')
                s.shot('arrange-menu-' + str(row))
                s.key('Return')
            try:
                paths = []
                for i, dimensions in enumerate(((160, 100), (80, 160), (210, 80), (100, 120), (130, 90), (90, 140))):
                    path = work / f'image-{i}.png'
                    Image.new('RGB', dimensions, (200-i*5, 40, 30)).save(path)
                    paths.append(path)
                pointer(200, 170)
                drop(s.window, paths[:1])
                wait(lambda: 'modified' in s.title() and 'Import ' not in s.title(), 'native import')
                before = s.save(board)
                x, y = image_point()
                drag((x, y), (x+65, y+35))
                moved = s.save(board)
                record('move changes persisted document', moved != before)
                s.key('ctrl+z')
                record('logical Ctrl+Z reaches native owner and restores document', s.save(board) == before)
                s.key('ctrl+shift+z')
                record('logical Ctrl+Shift+Z restores exact move', s.save(board) == moved)
                # Menu-open keyboard shortcut uses the same logical semantics.
                s.key('F10', 'ctrl+z')
                record('popup keyboard Undo restores same history', s.save(board) == before)
                s.key('ctrl+y')
                record('Ctrl+Y alias restores exact move', s.save(board) == moved)
                s.key('F10', 'Down', 'Down', 'Right', 'Return')
                record('menu Undo restores same operation', s.save(board) == before)
                s.key('ctrl+shift+z')
                s.save(board)
                s.key('t')
                pointer(40, 60)
                command('xdotool', 'click', '1')
                s.text('draft')
                s.key('ctrl+z')
                record('draft consumes document Undo', 'Ctrl+Enter confirm' in s.title() and saved_objects(board) == moved)
                s.key('ctrl+a')
                s.text('Native note')
                s.key('ctrl+Return')
                note = s.save(board)
                record('note commit returns Pointer and logical draft SelectAll replaces text', 'Pointer' in s.title() and any(o.get('text') == 'Native note' for o in note['objects']))
                s.key('F2')
                record('F2 opens note editing', 'Ctrl+Enter confirm' in s.title())
                s.key('Escape')
                command('xdotool', 'keydown', 'ctrl')
                command('xdotool', 'keydown', 'z')
                command('xdotool', 'keydown', 'z')
                command('xdotool', 'keyup', 'ctrl')
                command('xdotool', 'keyup', 'z')
                time.sleep(.2)
                record('repeat suppression and modifier release order undo note once', s.save(board) == moved)
                s.key('ctrl+shift+z')
                s.save(board)
                s.key('t')
                pointer(35, 280)
                command('xdotool', 'click', '1')
                s.text('cancel me')
                s.key('Escape')
                record('note cancel returns Pointer without history', 'Pointer' in s.title() and s.save(board) == note)
                for key, kind in (('r', 4), ('l', 6), ('a', 7)):
                    s.key(key)
                    drag((40, 390), (125, 440))
                    shaped = s.save(board)
                    record(key + ' creates shape and returns Pointer', 'Pointer' in s.title() and any(o['kind'] == kind for o in shaped['objects']))
                    s.key('ctrl+z')
                    record(key + ' creation is one Undo', s.save(board) == note)
                s.key('p')
                drag((40, 400), (125, 440))
                scribble = s.save(board)
                record('Freehand remains repeatable', 'Scribble' in s.title() and any(o['kind'] == 8 for o in scribble['objects']))
                s.key('v')
                record('V returns Pointer', 'Pointer' in s.title())
                s.key('ctrl+z')
                record('Freehand creation is one Undo', s.save(board) == note)
                s.key('r')
                pointer(35, 350)
                command('xdotool', 'mousedown', '1')
                pointer(150, 420)
                s.key('Escape')
                command('xdotool', 'mouseup', '1')
                record('Escape cancels uncommitted shape and returns Pointer', 'Pointer' in s.title() and s.save(board) == note)
                s.key('r')
                pointer(35, 350)
                command('xdotool', 'mousedown', '1')
                pointer(160, 420)
                import re
                root_window = re.search(r'Window id: (0x[0-9a-fA-F]+)', command('xwininfo', '-root')).group(1)
                command('xdotool', 'windowfocus', root_window)
                command('xdotool', 'mouseup', '1')
                s.focus()
                record('focus loss cancels uncommitted tool without history', 'Pointer' in s.title() and s.save(board) == note)
                s.key('ctrl+shift+f')
                record('frame creation returns Pointer', 'Pointer' in s.title() and any(o['kind'] == 'frame' for o in s.save(board)['objects']))
                s.key('ctrl+z')
                record('frame creation is one Undo', s.save(board) == note)
                s.key('g', 'shift+g')
                # Select-all/delete reaches exact board authority on AZERTY too.
                s.key('ctrl+a', 'Delete')
                record('logical Select All and Delete affect all objects', not s.save(board)['objects'])
                s.key('ctrl+z')
                record('deletion is one Undo', s.save(board) == note)
                drop(s.window, paths[1:])
                wait(lambda: 'modified' in s.title() and 'Import ' not in s.title(), 'multi import')
                unarranged = s.save(board)
                s.key('ctrl+a')
                arrange()
                arranged = s.save(board)
                record('Arrange in Grid changes both axes and preserves sizes', arranged != unarranged and {o['id']: o['size'] for o in arranged['objects']} == {o['id']: o['size'] for o in unarranged['objects']} and len({round(o['center'][1], 3) for o in arranged['objects']}) > 1)
                s.key('ctrl+z')
                record('Arrange in Grid is one exact Undo', s.save(board) == unarranged)
                s.key('ctrl+shift+z')
                record('Arrange Redo restores exact arrangement', s.save(board) == arranged)
                s.key('ctrl+a')
                arrange(1)
                snapped = s.save(board)
                record('Snap Selection changes arrangement in one atomic action', snapped != arranged)
                s.key('ctrl+z')
                record('Snap Selection is one exact Undo', s.save(board) == arranged)
                s.key('ctrl+shift+z')
                record('Snap Selection Redo exact', s.save(board) == snapped)
                s.key('F10', 'Down', 'Down', 'Right')
                for _ in range(5):
                    s.key('Down')
                s.key('Return')
                s.shot('keymap')
                s.text('Undo')
                s.key('Return', 'ctrl+s')
                s.shot('keymap-conflict')
                s.key('Escape', 'Escape')
                s.key('ctrl+z')
                record('conflicting native key capture refused and original Undo preserved', s.save(board) == arranged)
                s.key('ctrl+shift+z')
                s.save(board)
                s.key('F10', 'Down', 'Down', 'Right')
                for _ in range(5):
                    s.key('Down')
                s.key('Return')
                s.text('Undo')
                s.key('Delete', 'Return', 'ctrl+w')
                s.shot('keymap-remapped')
                s.key('Escape', 'Escape')
                s.key('ctrl+z')
                record('native remap removes previous Undo shortcut', s.save(board) == snapped)
                s.key('ctrl+w')
                record('native logical remap reaches shared Undo', s.save(board) == arranged)
                s.key('ctrl+shift+z')
                record('native Redo after remapped Undo restores exact state', s.save(board) == snapped)
                s.save(board)
                s.close()
                report = json.loads(s.report.read_text())
                record('native viewport exact', report['window_size'] == list(map(int, size.split('x'))))
                record('grid and snapping shortcuts dispatched', report['spatial']['grid'] and report['spatial']['snapping'])
                record('native backend GPU with no supply errors', report['backend'] == 'Vulkan' and report['errors'] == 0)
                # Native traces retain actual physical/logical/modifiers/normalized events
                # and resolved/owner-dispatched semantic actions, including AZERTY.
                trace = (work / 'input.log').read_text()
                record('native Undo dispatch traced', 'action: Undo' in trace and 'logical=Character("z")' in trace)
                if layout == 'fr':
                    record('AZERTY Z is physical W in native normalization', 'physical=Code(KeyW) logical=Character("z")' in trace)
                reopened = Session(binary, work, 'reopen', ['open', board], env)
                try:
                    record('Save and reopen preserve arrangement authority', reopened.save(board) == snapped)
                    reopened.close()
                    record('reopened native supply has no errors', json.loads(reopened.report.read_text())['errors'] == 0)
                finally:
                    reopened.kill()
                rows.append({'size': size, 'layout': layout, 'checks': checks,
                             'final_state': snapped, 'report_sha256': digest(s.report),
                             'captures': {p.name: digest(p) for p in work.glob('input-*.png')}})
                (root / 'summary.json').write_text(json.dumps({'binary_sha256': digest(binary), 'harness_sha256': digest(__file__), 'scope': 'native X11/XKB us/fr automation, NVIDIA Vulkan; no human comfort or Windows desktop claim', 'rows': rows}, indent=2) + '\n')
                print(size, layout, len(checks), 'PASS', flush=True)
            finally:
                s.kill()
    command('setxkbmap', '-layout', 'us')


if __name__ == '__main__':
    main()
