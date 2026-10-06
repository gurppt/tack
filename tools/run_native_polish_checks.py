#!/usr/bin/env python3
"""Phase 1J: owned X11 clipboard/menu/scale/theme checks, small reusable fixtures."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time
from PIL import Image
from run_local_production import Session, wait
from run_native_image_checks import command
from run_image_interaction import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--probe', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0') or os.environ.get('TACK_NATIVE_NO_WM') != '1':
        parser.error('requires an owned isolated X11 display and TACK_NATIVE_NO_WM=1')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary, probe = args.binary.resolve(), args.probe.resolve()
    helper = command('which', 'xclip').strip()
    checks, clipboard_cases, sessions, owners = [], [], [], []
    receipt = {'scope': 'automated native X11 events on owned display; no owner aesthetic feedback; no Windows/Wayland runtime claim',
               'binary_sha256': digest(binary), 'probe_sha256': digest(probe), 'helper': helper,
               'helper_sha256': digest(helper), 'display': os.environ['DISPLAY'], 'checks': checks, 'clipboard': clipboard_cases}
    def persist():
        (root / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    def record(name, observed, detail=None):
        checks.append({'name': name, 'observed': bool(observed), 'detail': detail})
        persist()
        if not observed:
            raise AssertionError(name)
    env = dict(os.environ, TACK_PROFILE_DIR=str(root / 'profile'), XDG_DATA_HOME=str(root / 'data'),
               TACK_TEST_WINDOW_SIZE='800x600', GSETTINGS_BACKEND='memory', GIO_USE_VFS='local', GSK_RENDERER='cairo')
    png, jpg = root / 'source.png', root / 'source.jpg'
    Image.new('RGB', (96, 72), (70, 125, 180)).save(png)
    Image.new('RGB', (80, 120), (180, 105, 70)).save(jpg)
    (root / 'bad.png').write_bytes(b'invalid image header')
    def clipboard(data, mime):
        for owner in owners:
            if owner.poll() is None:
                owner.terminate(); owner.wait(timeout=3)
        owners.clear()
        owner = subprocess.Popen([helper, '-selection', 'clipboard', '-in', '-quiet', '-target', mime],
                                 stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        owner.stdin.write(data)
        owner.stdin.close()
        owners.append(owner)
        time.sleep(.15)
    def diagnose(name, text=False, override=None):
        result = subprocess.run([str(probe), str(root / 'probe-work'), *(['text'] if text else [])],
                                env=override or env, capture_output=True, text=True, timeout=15, check=True)
        value = json.loads(result.stdout)
        clipboard_cases.append({'case': name, 'session': 'x11', 'helper': helper, 'normalized': value,
                                'backend_mime_worker_trace': result.stderr.strip(), 'worker_result': value['kind']})
        persist()
        return value
    try:
        board = root / 'board.tack'
        s = Session(binary, root, 'native', ['new', board], env)
        sessions.append(s)
        def paste(name, data, mime, expected, by_menu=False):
            clipboard(data, mime)
            normalized = diagnose(name)
            if by_menu:
                command('xdotool', 'mousemove', '--window', s.window, '700', '500')
                command('xdotool', 'click', '3')
                # Six application entries + first canvas row (headings skipped).
                s.key(*(['Down'] * 7), 'Return')
            else:
                s.key('ctrl+v')
            wait(lambda: 'modified' in s.title() and 'Import ' not in s.title(), name + ' admitted')
            saved = s.save(board)
            count = len(saved['objects'])
            clipboard_cases[-1]['imported_total'] = count
            clipboard_cases[-1]['app_title'] = s.title()
            record(name, count == expected, normalized)
            return saved
        screenshot = paste('PNG screenshot Ctrl+V', png.read_bytes(), 'image/png', 1)
        inspection = json.loads(command(str(binary), 'inspect', str(board)))
        record('screenshot uses embedded source', inspection['embedded'] == 1, inspection)
        paste('single file-manager PNG URI', png.as_uri().encode(), 'text/uri-list', 2)
        paste('single JPEG URI', jpg.as_uri().encode(), 'text/uri-list', 3)
        paste('multiple copied files', ('copy\n' + png.as_uri() + '\n' + jpg.as_uri()).encode(), 'x-special/gnome-copied-files', 5)
        mixed = (png.as_uri() + '\nhttps://example.test/no.png\n' + (root / 'bad.png').as_uri() + '\n' + jpg.as_uri()).encode()
        clipboard(mixed, 'text/uri-list')
        diagnose('mixed valid, remote, invalid image')
        s.key('ctrl+v')
        wait(lambda: '2 admitted, 2 rejected' in s.title(), 'one bounded rejection summary')
        saved = s.save(board)
        record('mixed batch keeps valid images with summarized rejects', len(saved['objects']) == 7, s.title())
        paste('context menu Paste same semantic path', png.read_bytes(), 'image/png', 8, True)
        saved = paste('plain text creates note', b'/review notes\nsecond line', 'UTF8_STRING', 9)
        record('text is a durable note', saved['objects'][-1]['kind'] == 3)
        # Existing note editor text paste remains bounded and commits through note history.
        s.key('t')
        command('xdotool', 'mousemove', '--window', s.window, '25', '380')
        command('xdotool', 'click', '1')
        wait(lambda: 'Ctrl+Enter confirm' in s.title(), 'note editor')
        clipboard(b'editor clipboard text', 'UTF8_STRING'); diagnose('text into existing editor', True)
        s.key('ctrl+v'); time.sleep(.3); s.key('ctrl+Return')
        saved = s.save(board)
        record('editor text paste and confirmation', len(saved['objects']) == 10)
        paste('JPEG clipboard payload', jpg.read_bytes(), 'image/jpeg', 11)
        # Unsupported/remote content is explicit and cannot mutate the document.
        before = digest(board)
        for name, mime, data, message in [('unsupported MIME', 'application/octet-stream', b'unsupported', 'Unsupported clipboard format'),
                                           ('remote URI refused', 'text/uri-list', b'https://example.test/no.png', 'No supported local clipboard files')]:
            clipboard(data, mime); value = diagnose(name)
            s.key('ctrl+v'); wait(lambda: message in s.title(), name + ' useful error')
            record(name, value['kind'] == 'error' and digest(board) == before, s.title())
            s.key('Escape')
        value = diagnose('missing helper', override=dict(env, PATH='/nonexistent'))
        record('missing helper actionable', 'install xclip' in value.get('error', ''))
        wayland = dict(env, PATH='/nonexistent', WAYLAND_DISPLAY='wayland-owned')
        wayland.pop('DISPLAY', None); wayland.pop('XDG_SESSION_TYPE', None)
        value = diagnose('pure Wayland missing helper', override=wayland)
        record('pure Wayland missing helper actionable without session type', 'install wl-clipboard' in value.get('error', ''))
        missing = dict(env, PATH='/nonexistent'); missing.pop('DISPLAY', None); missing.pop('WAYLAND_DISPLAY', None)
        value = diagnose('missing desktop backend', override=missing)
        record('missing desktop actionable', 'desktop session' in value.get('error', ''))
        clipboard(b'x' * (64 * 1024 * 1024 + 1), 'image/png')
        value = diagnose('oversize clipboard PNG')
        record('oversize refuses before staging/decode', 'size limit' in value.get('error', ''))
        clipboard(png.read_bytes(), 'image/png')
        cancelled = subprocess.run([str(probe), str(root / 'cancel-work'), 'worker-cancel'], env=env, capture_output=True, text=True, timeout=16, check=True)
        value = json.loads(cancelled.stdout)
        record('cancel after read discards result and removes staging', value['staged_before'] and not value['staged_after'] and value['delivered'] == 0, value)
        # Direct scale and theme selection through real native preferences.
        profile_path = root / 'profile/preferences.json'
        def preferences():
            s.key('Escape', 'ctrl+comma')
        def choose_scale(current, target):
            preferences(); s.key(*(['Down'] * 3), 'Return')
            s.key(*(['Down'] * (target-current) if target >= current else ['Up'] * (current-target)), 'Return')
            wait(lambda: profile_path.exists() and json.loads(profile_path.read_text())['ui_scale'] == target, 'scale persisted')
            s.shot('scale-' + str(target)); s.key('Escape')
        def choose_theme(current, target):
            preferences(); s.key(*(['Down'] * 4), 'Return')
            s.key(*(['Down'] * (target-current) if target >= current else ['Up'] * (current-target)), 'Return')
            theme = ('VeryDark', 'NeutralGray', 'Light')[target]
            wait(lambda: profile_path.exists() and json.loads(profile_path.read_text())['theme'] == theme, 'theme persisted')
            s.key('Escape'); s.shot('theme-' + theme)
            command('xdotool', 'mousemove', '--window', s.window, '10', '10'); command('xdotool', 'click', '3'); time.sleep(.15)
            s.shot('menu-' + theme); s.key(*(['Down'] * 4), 'Right'); s.shot('tools-' + theme); s.key('Escape')
        choose_scale(0,2)
        record('scale increase to 2x', json.loads(profile_path.read_text())['ui_scale'] == 2)
        command('xdotool', 'mousemove', '--window', s.window, '10', '10'); command('xdotool', 'click', '3'); time.sleep(.2)
        s.shot('combined-2x'); s.key('Down', 'Right'); s.shot('File-2x')
        # Pair at left edge uses adjacent bounded columns; select parent Preferences by pointer.
        command('xdotool', 'mousemove', '--window', s.window, '40', '196')
        time.sleep(.15); command('xdotool', 'click', '1'); time.sleep(.15)
        s.shot('parent-preferences-2x'); s.key('Escape')
        choose_theme(1,0); choose_theme(0,1)
        choose_scale(2,1)
        record('scale decreases to 1x', json.loads(profile_path.read_text())['ui_scale'] == 1)
        for target,current in [(0,1),(2,0),(1,2)]:
            choose_theme(current,target)
        record('exactly three themes round-trip native profile', json.loads(profile_path.read_text())['theme'] == 'NeutralGray')
        record('theme/scale do not change board bytes', digest(board) == before)
        command('xdotool', 'mousemove', '--window', s.window, '10', '10')
        command('xdotool', 'click', '3'); time.sleep(.2); s.shot('combined-canvas-1x'); s.key('Escape')
        s.key('F10'); s.shot('F10-1x'); s.key(*(['Down'] * 6), 'Return'); s.shot('keymap-from-F10')
        s.text('Paste'); s.key('Delete')
        wait(lambda: all(b['action'] != 'Paste' for b in json.loads(profile_path.read_text())['keymap']), 'Keymap reached and Paste unassigned')
        record('application and keymap remain reachable and editable', all(b['action'] != 'Paste' for b in json.loads(profile_path.read_text())['keymap']))
        s.key('F5')
        wait(lambda: any(b['action'] == 'Paste' for b in json.loads(profile_path.read_text())['keymap']), 'Keymap Paste restored')
        s.key('Escape')
        time.sleep(.5)
        task_names = [p.read_text().strip() for p in Path(f'/proc/{s.process.pid}/task').glob('*/comm')]
        children = Path(f'/proc/{s.process.pid}/task/{s.process.pid}/children').read_text().split()
        record('clipboard worker/helper released after use', not children and not any(n.startswith(('tack-local-oper', 'tack-helper')) for n in task_names), {'tasks':task_names,'children':children})
        s.close(); sessions.remove(s)
        # Restart demonstrates persisted values, native output exposes scale.
        s = Session(binary, root, 'restart', ['open', board], env); sessions.append(s)
        s.shot('restored-profile'); s.close(); sessions.remove(s)
        restart = json.loads((root / 'restart.json').read_text())
        record('profile restart preserves scale', restart['local']['ui_scale'] == 1)
        for size in ['1024x768', '1600x900']:
            large_env = dict(env, TACK_TEST_WINDOW_SIZE=size)
            s = Session(binary, root, 'size-' + size, ['open', board], large_env); sessions.append(s)
            command('xdotool', 'mousemove', '--window', s.window, '10', '10'); command('xdotool', 'click', '3'); time.sleep(.15)
            s.shot('combined'); s.key('Escape', 'ctrl+comma'); s.shot('preferences'); s.key(*(['Down'] * 4), 'Return'); s.shot('themes'); s.key('Escape', 'Escape')
            s.close(); sessions.remove(s)
            output = json.loads((root / ('size-' + size + '.json')).read_text())
            record('native viewport ' + size, output['window_size'] == list(map(int,size.split('x'))))
        print(json.dumps({'checks':len(checks), 'passed':sum(c['observed'] for c in checks)}, indent=2))
    finally:
        for session in sessions:
            session.kill()
        for owner in owners:
            if owner.poll() is None:
                owner.terminate(); owner.wait(timeout=3)
        # Diagnostic staging has no document authority; retain no copied image bytes.
        for path in (root / 'probe-work').glob('clipboard.*'):
            path.unlink()
        persist()

if __name__ == '__main__':
    main()
