#!/usr/bin/env python3
"""Owned 800x600 toolbar events, persistence and restart-only pixel icon witness."""
import argparse
import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
from PIL import Image
from run_local_production import Session, wait
from run_native_annotation_checks import saved_objects
from run_native_image_checks import command
from run_image_interaction import digest


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--profile', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0') or os.environ.get('TACK_NATIVE_NO_WM') != '1':
        p.error('Owned isolated display required')
    root = a.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = a.binary.resolve()
    profile = json.loads(a.profile.read_text())
    profile.update(recent=[], ui_scale=1)
    checks, sessions = [], []
    icons = root / 'icons'
    shutil.copytree(Path(__file__).resolve().parents[1] / 'gfx/icons', icons)

    def record(name, ok, detail=None):
        checks.append(dict(name=name, observed=bool(ok), detail=detail))
        (root / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
        if not ok:
            raise AssertionError(name)

    def start(name, settings=None, reuse=None, arguments=None):
        path = root / (reuse or name) / 'profile'
        if reuse is None:
            path.mkdir(parents=True)
            (path / 'preferences.json').write_text(json.dumps(settings or profile))
        env = dict(os.environ, TACK_PROFILE_DIR=str(path), TACK_ICON_DIR=str(icons),
                   TACK_TEST_WINDOW_SIZE='800x600', WINIT_X11_SCALE_FACTOR='1', TACK_TRACE_INPUT='1')
        s = Session(binary, root, name, arguments or ['new', root / f'{name}.tack'], env)
        sessions.append(s)
        time.sleep(.3)
        return s, path / 'preferences.json'

    def click(s, x, y):
        s.focus()
        command('xdotool', 'mousemove', '--window', s.window, str(x), str(y))
        command('xdotool', 'click', '1')
        time.sleep(.15)

    def persisted(path, field, expected):
        return wait(lambda: json.loads(path.read_text())['toolbar'][field] == expected,
                    f'persist {field}={expected}')

    def capture(s, label):
        s.shot(label)
        return Image.open(root / f'{s.name}-{label}.png').convert('RGB')

    try:
        s, prefs = start('editor')
        s.key('F10')
        persisted(prefs, 'placement', 'Hidden')
        record('Toggle Toolbar hides and persists', True)
        s.key('F10')
        persisted(prefs, 'placement', 'Top')
        s.key('F8')
        for place in ['Left', 'Right', 'Bottom', 'Floating', 'Hidden', 'Top']:
            capture(s, 'edit-before-' + place.lower())
            click(s, 100, 422)
            persisted(prefs, 'placement', place)
            s.key('Escape')
            capture(s, place.lower())
            # Resize within the owned display and return to minimum client size.
            command('xdotool', 'windowsize', s.window, '1024', '768')
            time.sleep(.2)
            command('xdotool', 'windowsize', s.window, '800', '600')
            time.sleep(.3)
            s.focus()
            s.key('F8')
            time.sleep(.2)
        record('all six placements survive GUI edit and resize', True)
        old = json.loads(prefs.read_text())['toolbar']['actions']
        click(s, 90, 397)
        wait(lambda: json.loads(prefs.read_text())['toolbar']['actions'] != old, 'checkbox action toggles')
        click(s, 90, 397)
        wait(lambda: len(json.loads(prefs.read_text())['toolbar']['actions']) == 12, 're-add selected action')
        changed = json.loads(prefs.read_text())['toolbar']['actions']
        click(s, 280, 397)
        moved = wait(lambda: json.loads(prefs.read_text())['toolbar']['actions'] != changed, 'selected action reordered up')
        record('catalog Add/Remove and Up persist semantic IDs', bool(moved))
        click(s, 280, 422)
        persisted(prefs, 'actions', profile['toolbar']['actions'])
        record('Reset restores default toolbar', True)
        for _ in range(4):
            click(s, 100, 422)
        persisted(prefs, 'placement', 'Floating')
        s.key('Escape')
        command('xdotool', 'mousemove', '--window', s.window, '52', '60')
        command('xdotool', 'mousedown', '1')
        command('xdotool', 'mousemove', '--window', s.window, '202', '160')
        command('xdotool', 'mouseup', '1')
        persisted(prefs, 'floating', [198, 148])
        record('floating grip drag persists integer coordinates', True)
        s.close()
        restored, _ = start('restored', reuse='editor')
        capture(restored, 'floating-restored')
        restored.close()
        report = json.loads(restored.report.read_text())
        record('placement survives native restart', report['chrome']['placement'] == 'Floating', report['chrome'])

        drawing, _ = start('drawing')
        click(drawing, 88, 14)  # Existing Rectangle semantic action.
        command('xdotool', 'mousemove', '--window', drawing.window, '350', '250')
        command('xdotool', 'mousedown', '1')
        command('xdotool', 'mousemove', '--window', drawing.window, '430', '320')
        command('xdotool', 'mouseup', '1')
        first = drawing.save(root / 'drawing.tack')
        record('toolbar Rectangle invokes existing annotation semantics', len(first['objects']) == 1 and first['objects'][0]['kind'] == 4)
        drawing.key('ctrl+z')
        record('existing Undo reverses toolbar-created rectangle', not drawing.save(root / 'drawing.tack')['objects'])
        click(drawing, 20, 14)
        command('xdotool', 'mousemove', '--window', drawing.window, '350', '250')
        command('xdotool', 'mousedown', '2')
        command('xdotool', 'mousemove', '--window', drawing.window, '100', '14')
        command('xdotool', 'mouseup', '2')
        time.sleep(.2)
        released = drawing.title()
        command('xdotool', 'mousemove', '--window', drawing.window, '500', '400')
        time.sleep(.2)
        record('canvas pan release over toolbar ends held gesture', drawing.title() == released)
        click(drawing, 42, 14)
        left_begin_ms = (time.monotonic() - drawing.started) * 1000
        command('xdotool', 'mousemove', '--window', drawing.window, '350', '250')
        command('xdotool', 'mousedown', '1')
        command('xdotool', 'mousemove', '--window', drawing.window, '400', '300')
        command('xdotool', 'mouseup', '1')
        command('xdotool', 'mousemove', '--window', drawing.window, '500', '400')
        record('Pan toolbar tool changes camera without document edit', not drawing.save(root / 'drawing.tack')['objects'])
        drawing.close()
        pan_report = json.loads(drawing.report.read_text())
        before_pan = [f for f in pan_report['frames'] if f['elapsed_ms'] <= left_begin_ms][-1]['camera']
        after_pan = pan_report['frames'][-1]['camera']
        record('dedicated Pan uses existing camera pan', before_pan != after_pan, dict(before=before_pan, after=after_pan))

        scribble, _ = start('scribble')
        click(scribble, 154, 14)
        command('xdotool', 'mousemove', '--window', scribble.window, '350', '250', 'mousedown', '1',
                'mousemove', '--window', scribble.window, '100', '14', 'mouseup', '1')
        time.sleep(.2)
        trace = (root / 'scribble.log').read_text()
        record('scribble release over toolbar reaches semantic End', 'action: ImagePointer, phase: End(' in trace)
        command('xdotool', 'mousemove', '--window', scribble.window, '500', '400')
        scribbled = scribble.save(root / 'scribble.tack')['objects']
        record('cross-toolbar scribble commits one bounded stroke', len(scribbled) == 1 and scribbled[0]['kind'] == 8)
        scribble.close()

        imagepath = root / 'small.png'
        Image.new('RGB', (64, 64), (77, 150, 99)).save(imagepath)
        imageboard = root / 'image.tack'
        subprocess.run([str(binary), 'create', str(imageboard), '--embedded', str(imagepath)],
                       capture_output=True, check=True, timeout=10)
        original_center = saved_objects(imageboard)['objects'][0]['center']
        image, _ = start('image', arguments=['open', imageboard])
        image.key('ctrl+a')
        command('xdotool', 'mousemove', '--window', image.window, '400', '300', 'mousedown', '1',
                'mousemove', '--window', image.window, '100', '14', 'mouseup', '1')
        time.sleep(.2)
        record('image release over toolbar reaches semantic End',
               'action: ImagePointer, phase: End(' in (root / 'image.log').read_text())
        command('xdotool', 'mousemove', '--window', image.window, '500', '400')
        moved = image.save(imageboard)['objects'][0]['center']
        image.close()
        report = json.loads(image.report.read_text())
        zoom = report['frames'][0]['zoom']
        expected_center = [original_center[0]-300/zoom, original_center[1]-286/zoom]
        record('image stops at released position, later pointer does not drag',
               all(abs(a-b) < .01 for a,b in zip(moved, expected_center)), dict(actual=moved, expected=expected_center))

        pattern = Image.new('RGBA', (16, 16))
        pattern.putdata([(239, 31, 73, 255) if (x+y) % 2 else (17, 211, 61, 255)
                         for y in range(16) for x in range(16)])
        pattern.save(icons / 'arrow.png')
        for scale in [1, 2]:
            settings = copy.deepcopy(profile)
            settings['ui_scale'] = scale
            settings['keymap'] = [b for b in settings['keymap'] if b['action'] != 'SelectTool(Arrow)'
                and b['control'] != {'LogicalKey': {'Named': 'F11'}}]
            settings['keymap'].append(dict(action='SelectTool(Arrow)', control={'LogicalKey': {'Named': 'F11'}},
                modifiers=0, modifier_match='Exact', trigger='Press'))
            icon, _ = start(f'icon-{scale}', settings)
            command('xdotool', 'mousemove', '--window', icon.window, str(130*scale), str(14*scale))
            time.sleep(.2)
            image = capture(icon, 'edited-png')
            actual = image.crop((122*scale, 6*scale, 138*scale, 22*scale))
            expected = pattern.convert('RGB').resize((16*scale, 16*scale), Image.Resampling.NEAREST)
            record(f'edited PNG after restart is exact nearest at {scale}x', actual.tobytes() == expected.tobytes())
            icon.close()
        (icons / 'arrow.png').write_bytes(b'corrupt owned PNG')
        fallback, _ = start('fallback')
        capture(fallback, 'placeholder')
        fallback.close()
        record('corrupt PNG native startup survives with diagnostic', '[tack/icons] arrow:' in (root / 'fallback.log').read_text())
        (root / 'receipt.json').write_text(json.dumps(dict(binary_sha256=digest(binary),
            harness_sha256=digest(__file__), checks=checks), indent=2) + '\n')
    finally:
        for s in sessions:
            s.kill()
    print(root / 'receipt.json')


if __name__ == '__main__':
    main()
