#!/usr/bin/env python3
"""Owned-display native corrective workflows; never uses a user's profile."""
import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import time

from PIL import Image
from run_local_production import Session, wait
from run_native_image_checks import command
from run_native_annotation_checks import saved_objects
from run_image_interaction import digest
from run_phase1l_local import descendants


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'profile', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        parser.error('Owned isolated display required')
    with socket.socket() as probe:
        probe.bind(('0.0.0.0', 7337))
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    profile_path = root / 'profile' / 'preferences.json'
    profile_path.parent.mkdir()
    profile = json.loads(args.profile.read_text())
    profile.update(recent=[], local_views=[], ui_scale=1, status_bar=False)
    remaps = {'F2': 'EditToolbar', 'F3': 'KeymapEditor', 'F4': 'SaveToLocal',
              'F6': 'ShareBoard', 'F7': 'StopSharing', 'F11': 'CloseBoard',
              'F12': 'CopySharedBoardAddress'}
    profile['keymap'] = [b for b in profile['keymap']
                         if b['action'] not in remaps.values()
                         and b['control'] not in [{'LogicalKey': {'Named': k}} for k in remaps]]
    for key, action in remaps.items():
        profile['keymap'].append(dict(action=action, control={'LogicalKey': {'Named': key}},
                                      modifiers=0, modifier_match='Exact', trigger='Press'))
    profile_path.write_text(json.dumps(profile))
    env = dict(os.environ, TACK_PROFILE_DIR=str(profile_path.parent),
               TACK_NATIVE_NO_WM='1', TACK_NATIVE_DIAGNOSTICS='1',
               TACK_TEST_WINDOW_SIZE='800x600', WINIT_X11_SCALE_FACTOR='1',
               GSETTINGS_BACKEND='memory', GIO_USE_VFS='local', TACK_TRACE_INPUT='1')
    checks = []

    def record(name, ok, detail=None):
        checks.append(dict(name=name, observed=bool(ok), detail=detail))
        (root / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
        if not ok:
            raise AssertionError(name)

    def cli(*arguments):
        result = subprocess.run([str(binary), *map(str, arguments)], env=env,
                                capture_output=True, text=True, timeout=30)
        if result.returncode:
            raise AssertionError(result.stderr)
        return json.loads(result.stdout)

    def click(x, y, count=1):
        command('xdotool', 'mousemove', '--window', session.window, str(x), str(y))
        command('xdotool', 'click', '--repeat', str(count), '--delay', '100', '1')
        time.sleep(.15)

    image = root / 'image.png'
    Image.new('RGB', (160, 100), (45, 130, 185)).save(image)
    original = root / 'original.tack'
    cli('create', original, '--embedded', image)
    original_hash = digest(original)
    original_id = cli('inspect', original)['document_id']
    session = Session(binary, root, 'artist', ['open', original], env)
    try:
        time.sleep(1)
        session.key('Escape')
        placements = []
        for expected in ('Top', 'Left', 'Right', 'Bottom', 'Floating'):
            for _ in range(4):
                session.key('F9')
            time.sleep(.3)
            stored = json.loads(profile_path.read_text())
            placements.append(stored['toolbar']['placement'])
            record('native repeated toolbar toggle ' + expected,
                   stored['toolbar']['placement'] == expected and session.process.poll() is None)
            session.key('F2')
            session.shot('toolbar-' + expected)
            click(90, 424)  # Position button, second footer row.
            session.key('Escape')
        # Floating -> Hidden -> restore Floating -> editor Reset -> Top.
        session.key('F9', 'F9', 'F2')
        click(240, 424)
        session.key('Escape')
        record('toolbar editor reset returns centered Top',
               wait(lambda: json.loads(profile_path.read_text())['toolbar']['placement'] == 'Top',
                    'toolbar Reset'))
        # Direct shortcut capture with collision, using the native Keymap search.
        session.key('F3')
        session.text('lock annotation selection')
        session.shot('keymap-filter')
        click(450, 92, count=2)
        session.shot('keymap-capture')
        session.key('F9')
        session.shot('keymap-reassigned')
        session.key('Escape', 'Escape')
        time.sleep(.3)
        bindings = json.loads(profile_path.read_text())['keymap']
        record('double-click capture reassigns collision and unbinds old action',
               any(b['action'] == 'ToggleAnnotationSelectionLock'
                   and b['control'] == {'LogicalKey': {'Named': 'F9'}} for b in bindings)
               and not any(b['action'] == 'ToggleToolbar' for b in bindings))
        # Share current view through the real native picker.
        session.key('F6', 'Return')
        shared = root / 'original-shared.tack'
        session.picker('Return', shared)
        session.focus()
        wait(lambda: ' · Connected ·' in session.title(), 'current window Share connection', seconds=20)
        server = wait(lambda: next((p for p in descendants(session.process.pid)
                                  if Path(f'/proc/{p}/comm').read_text().strip() == 'tack-server'), None),
                      'owned hosted server')
        metadata = json.loads(Path(str(shared) + '.sharing.json').read_text())
        record('Share transitions current view and preserves local original',
               metadata['board'] != original_id and digest(original) == original_hash)
        session.shot('shared-invite')
        session.key('Return')
        time.sleep(.4)
        session.shot('copied')
        clipboard = subprocess.run(['xclip', '-selection', 'clipboard', '-out'],
                                   capture_output=True, text=True, timeout=3)
        record('Copy returns visible canonical invite', clipboard.stdout == metadata['invite'])
        time.sleep(1.5)
        session.key('Escape')
        session.shot('shared-bottom-status-pref-hidden')
        # Stop checkpoints/reaps and immediately explains read-only offline state.
        session.key('F7')
        wait(lambda: not Path(f'/proc/{server}').exists(), 'managed server reaped', seconds=20)
        session.shot('offline-centered')
        session.key('Escape')
        local = root / 'independent.tack'
        session.picker('F4', local)
        session.focus()
        wait(lambda: local.exists() and ' · Connected ·' not in session.title(),
             'local independent copy installed', seconds=20)
        record('Save to Local has fresh identity, originals and no shared companion',
               cli('inspect', local)['document_id'] != metadata['board']
               and not Path(str(local) + '.sharing.json').exists()
               and Path(str(local) + '.assets').is_dir() and digest(original) == original_hash)
        session.key('ctrl+a', 'ctrl+d', 'ctrl+s')
        wait(lambda: len(saved_objects(local)['objects']) == 2, 'local fork editable and saved')
        record('local copy accepts edits without server', not Path(f'/proc/{server}').exists())
        session.key('F11')
        wait(lambda: '0 selected' in session.title(), 'Close Board empty window')
        record('Close Board retains application process', session.process.poll() is None)
        session.shot('closed-board-empty')
        # Basic Note editing: Enter validates; Shift+Enter inserts newline.
        session.key('t')
        command('xdotool', 'mousemove', '--window', session.window, '180', '180')
        command('xdotool', 'mousedown', '1')
        command('xdotool', 'mousemove', '--window', session.window, '400', '320')
        command('xdotool', 'mouseup', '1')
        time.sleep(.2)
        session.text('First')
        session.key('shift+Return')
        session.text('Second')
        session.key('Return')
        notes = root / 'note.tack'
        session.picker('ctrl+shift+s', notes)
        wait(lambda: notes.exists(), 'validated Note saved')
        objects = saved_objects(notes)['objects']
        record('Note Shift+Enter newline and Enter completion',
               len(objects) == 1 and objects[0].get('text') == 'First\nSecond', objects)
        session.close()
        (root / 'receipt.json').write_text(json.dumps(dict(
            binary_sha256=digest(binary), harness_sha256=digest(Path(__file__)),
            scope='Native Linux GPU on owned isolated display; physical LAN/Windows pending',
            checks=checks), indent=2) + '\n')
    finally:
        session.kill()
    print(root / 'receipt.json')


if __name__ == '__main__':
    main()
