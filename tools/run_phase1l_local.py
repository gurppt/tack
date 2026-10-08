#!/usr/bin/env python3
"""Phase 1L local workflows on an explicitly isolated X11 display and owned files."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import time
from PIL import Image
from run_local_production import Session, wait
from run_native_annotation_checks import saved_objects
from run_native_image_checks import command
from run_idle import close_owned_window
from run_image_interaction import digest


def visible_windows(pid):
    result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--pid', str(pid)],
                            capture_output=True, text=True, timeout=3)
    return sorted(result.stdout.splitlines())


def owned_picker_window(session):
    result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^Tack$'],
                            capture_output=True, text=True, timeout=3)
    for window in reversed(result.stdout.splitlines()):
        try:
            pid = int(command('xdotool', 'getwindowpid', window).strip())
            status = Path(f'/proc/{pid}/status').read_text().splitlines()
            parent = next(int(line.split()[1]) for line in status if line.startswith('PPid:'))
            # Winit children can briefly have the same title during startup.
            if parent == session.process.pid and Path(os.readlink(f'/proc/{pid}/exe')).name == 'zenity':
                return window
        except (FileNotFoundError, ProcessLookupError, subprocess.CalledProcessError):
            continue
    return None


def descendants(pid):
    """Only descendants of a process this harness launched; never global window cleanup."""
    result, pending = [], [pid]
    while pending:
        parent = pending.pop()
        try:
            children = Path(f'/proc/{parent}/task/{parent}/children').read_text().split()
        except FileNotFoundError:
            continue
        children = list(map(int, children))
        result.extend(children)
        pending.extend(children)
    return result


def native_directory(profile):
    descriptor = profile.get('last_board_directory')
    if descriptor is None:
        return None
    if descriptor['encoding'] != 1:
        raise AssertionError('X11 test expected a native Unix directory descriptor')
    return Path(os.fsdecode(bytes(descriptor['bytes'])))


def pick_existing_file(session, key, path):
    """Enter a complete file path without racing GTK's inline completion.

    Gradual typing can leave an autocompleted directory suffix in the entry.
    Focus the location field before selecting its contents and type atomically;
    never assume a fixed row in the asynchronous file list is the chosen file.
    """
    if not path.is_file():
        raise AssertionError(f'owned picker input is not a file: {path}')
    session.key(key)
    window = wait(session.picker_window, 'real owned zenity file picker')
    command('xdotool', 'windowfocus', '--sync', window)
    time.sleep(.25)
    session.key('ctrl+l')
    session.key('ctrl+a')
    command('xdotool', 'type', '--clearmodifiers', '--delay', '0', '--', str(path))
    time.sleep(.15)
    capture = session.root / f'{session.name}-picker-{len(session.pickers)}.png'
    subprocess.run(['import', '-window', window, str(capture)], check=True, timeout=4)
    geometry = dict(line.split('=', 1) for line in command(
        'xdotool', 'getwindowgeometry', '--shell', window).splitlines() if '=' in line)
    session.pickers.append({'key': key, 'geometry': geometry, 'capture': capture.name,
                            'entered_path': str(path), 'method': 'complete location path'})
    (session.root / f'{session.name}-pickers.json').write_text(
        json.dumps(session.pickers, indent=2) + '\n')
    session.key('Return')
    wait(lambda: not session.picker_window(), 'owned file picker accepted', seconds=20)
    session.focus()


def drag(session, start, end, shift=False):
    command('xdotool', 'mousemove', '--window', session.window, *map(str, start))
    if shift:
        command('xdotool', 'keydown', 'shift')
    try:
        command('xdotool', 'mousedown', '1')
        for step in range(1, 7):
            point = [round(a + (b-a)*step/6) for a, b in zip(start, end)]
            command('xdotool', 'mousemove', '--window', session.window, *map(str, point))
            time.sleep(.025)
    finally:
        command('xdotool', 'mouseup', '1')
        if shift:
            command('xdotool', 'keyup', 'shift')
    time.sleep(.15)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--source-export', action='store_true',
                        help='also exercise original export through a captured catalog binding')
    args = parser.parse_args()
    if os.environ.get('TACK_NATIVE_NO_WM') != '1' or os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        parser.error('requires isolated X11 display and TACK_NATIVE_NO_WM=1')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    env = dict(os.environ, TACK_TEST_WINDOW_SIZE='800x600', WINIT_X11_SCALE_FACTOR='1',
               XDG_DATA_HOME=str(root/'xdg-data'), XDG_CONFIG_HOME=str(root/'xdg-config'),
               GSETTINGS_BACKEND='memory', GIO_USE_VFS='local', GSK_RENDERER='cairo')
    sessions, checks = [], []

    def record(name, valid, detail=None):
        checks.append({'name': name, 'observed': bool(valid), 'detail': detail})
        (root/'checks.json').write_text(json.dumps(checks, indent=2)+'\n')
        if not valid:
            raise AssertionError(name)

    def start(name, arguments, profile):
        owned = dict(env, TACK_PROFILE_DIR=str(profile))
        session = Session(binary, root, name, arguments, owned)
        session.picker_window = lambda: owned_picker_window(session)
        default_picker = session.picker
        session.picker = lambda key, path: (
            pick_existing_file(session, key, path)
            if key in ('ctrl+i', 'ctrl+shift+r', 'ctrl+o') else default_picker(key, path))
        sessions.append(session)
        return session

    def profile(path):
        return json.loads(path.read_text()) if path.exists() else None

    def fresh_seed(folder):
        paths = list(folder.glob('untitled-slot-*.tack'))
        if len(paths) != 1:
            raise AssertionError(f'expected exactly one owned fresh seed: {paths}')
        return paths[0]

    def preferences(session, row):
        session.key('ctrl+comma')
        if row:
            session.key(*(['Down']*row))

    boards = root/'boards.v3'
    boards.mkdir()
    image = boards/'source.v2.PNG'
    Image.new('RGB', (180, 120), (170, 60, 100)).save(image)
    board = boards/'production.tack'
    with (root/'create.log').open('w') as log:
        subprocess.run([str(binary), 'create', str(board), '--linked', str(image)],
                       stdout=log, stderr=subprocess.STDOUT, check=True, timeout=20)
    board_hash = digest(board)
    original = saved_objects(board)
    malformed = boards/'malformed.tack'
    malformed.write_bytes(b'not a Tack board')
    try:
        failed_profile = root/'failed-profile'
        failed = start('fresh-failure', [], failed_profile)
        seed = fresh_seed(failed_profile)
        seed_hash = digest(seed)
        preference_file = failed_profile/'preferences.json'
        previous_preferences = profile(preference_file)
        failed.picker('ctrl+o', malformed)
        wait(lambda: '[tack/local]' in (root/'fresh-failure.log').read_text(), 'failed Open reported')
        failed.shot('failed-open')
        record('failed Open retains the usable fresh window and seed',
               failed.process.poll() is None and 'Untitled' in failed.title() and digest(seed) == seed_hash
               and visible_windows(failed.process.pid) == [failed.window] and not descendants(failed.process.pid))
        failed.key('Escape', 'ctrl+o')
        dialog = wait(failed.picker_window, 'fresh board remains usable after failed Open')
        command('xdotool', 'windowfocus', '--sync', dialog)
        failed.key('Escape')
        wait(lambda: not failed.picker_window(), 'owned picker cancelled')
        failed.focus()
        failed.key('Escape')
        record('cancelled picker preserves fresh document and board-directory preference',
               'Untitled' in failed.title() and digest(seed) == seed_hash
               and profile(preference_file) == previous_preferences)
        failed.close()
        record('closing failed/cancelled fresh window retires only its empty seed', not seed.exists())

        opened_profile = root/'opened-profile'
        opened = start('fresh-open', [], opened_profile)
        seed = fresh_seed(opened_profile)
        pid, window = opened.process.pid, opened.window
        opened.picker('ctrl+o', board)
        wait(lambda: board.name in opened.title() and 'Untitled' not in opened.title(), 'board installed into fresh window')
        wait(lambda: not seed.exists(), 'previous empty seed retired after successful Open')
        record('fresh Open reuses the same process and single window',
               opened.process.pid == pid and opened.window == window and opened.process.poll() is None
               and visible_windows(pid) == [window] and not descendants(pid),
               {'pid': pid, 'window': window, 'title': opened.title()})
        preference_file = opened_profile/'preferences.json'
        wait(lambda: preference_file.exists() and native_directory(profile(preference_file)) == boards,
             'successful Open directory persisted')
        record('successful Open remembers the board directory', native_directory(profile(preference_file)) == boards)
        opened.shot('same-window-open')
        opened.key('ctrl+o')
        dialog = wait(opened.picker_window, 'remembered folder picker')
        picker_pid = int(command('xdotool', 'getwindowpid', dialog).strip())
        picker_args = Path(f'/proc/{picker_pid}/cmdline').read_bytes().split(b'\0')
        record('next native board picker receives the successful board directory',
               any(os.fsencode(str(boards)+'/') in arg for arg in picker_args),
               {'directory': str(boards)})
        command('xdotool', 'windowfocus', '--sync', dialog)
        opened.key('Escape')
        wait(lambda: not opened.picker_window(), 'remembered picker cancelled')
        opened.focus()
        opened.key('Escape')
        raw = root/'foo.bar'
        target = root/'foo.bar.tack'
        opened.picker('ctrl+shift+s', raw)
        wait(lambda: target.exists() and target.name in opened.title(), '.tack Save As publication')
        record('Save As appends .tack to an arbitrary dotted filename',
               not raw.exists() and saved_objects(target) == original and digest(board) == board_hash)
        wait(lambda: native_directory(profile(preference_file)) == root, 'Save As directory persisted')
        record('successful Save As updates the next board folder', native_directory(profile(preference_file)) == root)
        opened.close()

        settings_profile = root/'settings-profile'
        settings_board = root/'settings.tack'
        settings = start('preferences-notes', ['new', settings_board], settings_profile)
        preference_file = settings_profile/'preferences.json'
        wait(preference_file.exists, 'initial board profile persisted')
        read = lambda: profile(preference_file)
        handle = read()['handle_size']
        preferences(settings, 4)
        settings.key('Return', 'Down', 'Return')
        wait(lambda: read()['theme'] == 'Light', 'first live Theme choice')
        settings.key('Up', 'Return')
        wait(lambda: read()['theme'] == 'NeutralGray', 'second live choice stays in Theme')
        settings.shot('theme-stays-open')
        settings.key('Escape', 'Down', 'Right')
        wait(lambda: read()['handle_size'] == handle+1, 'Theme Escape returned to Preferences')
        settings.key('Left', 'Escape')
        wait(lambda: read()['handle_size'] == handle, 'handle restored')
        record('live Theme remains open; Escape backs to Preferences then closes', True)
        preferences(settings, 7)
        settings.key('Return', 'Escape', 'Up', 'Up', 'Right')
        wait(lambda: read()['handle_size'] == handle+1, 'nested Keymap Escape returned to Preferences')
        settings.key('Left', 'Escape')
        wait(lambda: read()['handle_size'] == handle, 'nested Keymap restored preference')
        record('Keymap entered from Preferences backs to its parent', True)
        # The shared application menu has Keymap immediately before About.
        settings.key('F10', 'Up', 'Up', 'Return')
        settings.shot('direct-keymap')
        settings.key('Escape', 't')
        drag(settings, (80, 100), (300, 200))
        settings.text('Phase 1L controlled note wrapping and resize')
        settings.key('ctrl+Return', 'v')
        before = settings.save(settings_board)
        note = next(obj for obj in before['objects'] if obj['kind'] == 3)
        record('direct Keymap Escape closes and releases canvas input', len(before['objects']) == 1)
        drag(settings, (300, 200), (340, 220))
        resized = settings.save(settings_board)
        resized_note = next(obj for obj in resized['objects'] if obj['kind'] == 3)
        record('normal note corner drag changes box while retaining text size',
               resized_note['size'] != note['size'] and resized_note['font_size'] == note['font_size'])
        settings.key('ctrl+z')
        record('normal note drag is one history transaction', settings.save(settings_board) == before)
        drag(settings, (300, 200), (390, 245), shift=True)
        scaled = settings.save(settings_board)
        scaled_note = next(obj for obj in scaled['objects'] if obj['kind'] == 3)
        record('Shift note corner drag scales both box and text',
               scaled_note['size'] != note['size'] and scaled_note['font_size'] > note['font_size'])
        settings.key('ctrl+z')
        record('Shift note scaling undoes completely in one transaction', settings.save(settings_board) == before)
        settings.key('ctrl+y')
        record('Shift note scaling redoes exactly', settings.save(settings_board) == scaled)
        # Open from meaningful dirty work after the controlled note gesture tests.
        settings.key('ctrl+z')
        wait(lambda: 'modified' in settings.title(), 'note owns dirty state')
        owner_title = settings.title()
        settings.picker('ctrl+o', target)
        def opened_child():
            for pid in descendants(settings.process.pid):
                for window in visible_windows(pid):
                    title = command('xdotool', 'getwindowname', window)
                    if target.name in title and 'selected' in title:
                        return pid, window
            return None
        child_pid, child_window = wait(opened_child, 'nonempty Open separate board window')
        record('Open from a dirty note preserves the original document and window',
               settings.title() == owner_title and settings.process.poll() is None
               and settings.window != child_window and 'modified' in settings.title())
        close_owned_window(child_window)
        wait(lambda: not visible_windows(child_pid), 'separate opened window closes')
        settings.focus()
        record('dirty Open preserves unsaved note geometry and history',
               settings.save(settings_board) == before)
        settings.key('ctrl+y')
        record('note redo remains exact after separate-window Open',
               settings.save(settings_board) == scaled)
        settings.close()
        report = json.loads(settings.report.read_text())
        record('controlled note creation and final scaling own exactly two undo entries',
               report['editing']['undo_entries'] == 2 and not report['editing']['dirty'])

        if args.source_export:
            exported = start('original-export', ['open', target], opened_profile)
            exported.key('ctrl+a')
            preferences(exported, 7)
            exported.key('Return')
            exported.text('SaveOriginalAs')
            exported.key('Return', 'F8')
            wait(lambda: any(binding['action'] == 'SaveOriginalAs' and 'F8' in str(binding['control']) for binding in profile(opened_profile/'preferences.json')['keymap']),
                 'source export catalog binding persisted')
            exported.key('Escape', 'Escape', 'Escape')
            copy = root/'original-copy.PNG'
            exported.picker('F8', copy)
            wait(copy.exists, 'native original export publication')
            record('native Save Original As preserves the linked source hash and board',
                   digest(copy) == digest(image) and saved_objects(target) == original)
            exported.close()
    except Exception:
        for session in sessions:
            if session.process.poll() is None:
                try:
                    (root/f'{session.name}-failure-title.txt').write_text(session.title())
                    session.shot('failure')
                except (subprocess.SubprocessError, OSError):
                    pass
        raise
    finally:
        for session in sessions:
            if session.process.poll() is None:
                for child in reversed(descendants(session.process.pid)):
                    try:
                        os.kill(child, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
            session.kill()
    (root/'provenance.json').write_text(json.dumps({
        'binary_sha256': digest(binary), 'harness_sha256': digest(__file__),
        'display': os.environ['DISPLAY'], 'checks': len(checks),
        'input': 'owned generated files; actual X11 keys/pointer, native zenity pickers; no Windows or artist-feel claim',
    }, indent=2)+'\n')
    print(f'{len(checks)} Phase 1L native local checks passed', flush=True)


if __name__ == '__main__':
    main()
