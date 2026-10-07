#!/usr/bin/env python3
"""1F production workflows on an explicitly isolated X11 display and owned files."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time
from PIL import Image
from run_native_image_checks import command
from run_native_annotation_checks import saved_objects
from run_image_interaction import digest
from run_idle import close_owned_window, observe
from x11_drop import drop


def wait(predicate, description, seconds=8):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(.05)
    raise AssertionError(description)


class Session:
    def __init__(self, binary, root, name, arguments, env):
        self.root, self.name = root, name
        self.pickers = []
        self.report = root / f'{name}.json'
        self.log = (root / f'{name}.log').open('w')
        requested_size = env.get('TACK_TEST_WINDOW_SIZE')
        reporting = ['--output', str(self.report)] if arguments else []
        if arguments and requested_size:
            reporting += ['--window-size', requested_size]
        self.started = time.monotonic()
        self.process = subprocess.Popen([str(binary), *map(str, arguments), *reporting], env=env, stdout=self.log, stderr=subprocess.STDOUT)
        def mapped():
            if self.process.poll() is not None:
                raise RuntimeError(f'{name} exited {self.process.returncode}')
            result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--pid', str(self.process.pid)], capture_output=True, text=True, timeout=3)
            return result.stdout.splitlines()[-1] if result.stdout.strip() else None
        self.window = wait(mapped, 'owned window mapped')
        wait(lambda: 'selected' in self.title(), 'authoring title')
        if not arguments and requested_size:
            command('xdotool', 'windowsize', self.window, *requested_size.split('x'))
            time.sleep(.2)
        self.focus()

    def title(self):
        return command('xdotool', 'getwindowname', self.window)

    def focus(self):
        command('xdotool', 'windowfocus', '--sync', self.window)
        command('xdotool', 'keyup', 'ctrl', 'alt', 'shift', 'super')

    def key(self, *keys):
        command('xdotool', 'key', *keys)
        time.sleep(.1)

    def text(self, text):
        command('xdotool', 'type', '--clearmodifiers', '--', text)
        time.sleep(.1)

    def shot(self, name):
        subprocess.run(['import', '-window', self.window, str(self.root / f'{self.name}-{name}.png')], check=True, timeout=4)

    def picker_window(self):
        result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^Tack$'], capture_output=True, text=True, timeout=3)
        for window in reversed(result.stdout.splitlines()):
            try:
                pid = command('xdotool', 'getwindowpid', window).strip()
                parents = Path(f'/proc/{pid}/status').read_text().splitlines()
                parent = next(int(line.split()[1]) for line in parents if line.startswith('PPid:'))
                if parent == self.process.pid:
                    return window
            except (FileNotFoundError, ProcessLookupError, subprocess.CalledProcessError):
                continue
        return None

    def picker(self, key, path):
        self.key(key)
        dialog = self.picker_window
        window = wait(dialog, 'real owned zenity picker')
        geometry = dict(line.split('=', 1) for line in command('xdotool', 'getwindowgeometry', '--shell', window).splitlines() if '=' in line)
        capture = self.root / f'{self.name}-picker-{len(self.pickers)}.png'
        subprocess.run(['import', '-window', window, str(capture)], check=True, timeout=4)
        self.pickers.append({'key': key, 'geometry': geometry, 'capture': capture.name})
        (self.root / f'{self.name}-pickers.json').write_text(json.dumps(self.pickers, indent=2) + '\n')
        command('xdotool', 'windowfocus', '--sync', window)
        self.key('ctrl+l', 'ctrl+a')
        selects_file = key in ('ctrl+i', 'ctrl+shift+r', 'ctrl+o')
        self.text(str(path.parent) + '/' if selects_file else str(path))
        self.key('Return')
        time.sleep(.6)
        if selects_file and dialog():
            # GTK's location entry navigates a directory. Select the actual
            # file in the scoped native list, rather than accepting completion.
            self.key('ctrl+f')
            self.text(path.name)
            time.sleep(.6)
            if dialog():
                command('xdotool', 'mousemove', '--window', window, '250', '94')
                command('xdotool', 'click', '--repeat', '2', '--delay', '120', '1')
        if dialog():
            self.key('alt+o')
        wait(lambda: not dialog(), 'picker accepted', seconds=20)
        self.focus()

    def save(self, board):
        self.focus()
        self.key('ctrl+s')
        wait(lambda: 'saved' in self.title() and 'saving' not in self.title(), 'exact save ack')
        return saved_objects(board)

    def close(self, discard=False):
        self.focus()
        close_owned_window(self.window)
        time.sleep(.2)
        if self.process.poll() is None and discard:
            self.key('Down', 'Return')
        self.process.wait(timeout=8)
        self.log.close()
        if self.process.returncode:
            raise AssertionError(f'{self.name} close {self.process.returncode}')

    def kill(self):
        if self.process.poll() is None:
            self.process.kill()
            self.process.wait(timeout=4)
        self.log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('TACK_NATIVE_NO_WM') != '1' or os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        parser.error('requires isolated X11 display and TACK_NATIVE_NO_WM=1')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    env = dict(os.environ, TACK_PROFILE_DIR=str(root / 'profile'),
               XDG_DATA_HOME=str(root / 'xdg-data'), XDG_CONFIG_HOME=str(root / 'xdg-config'),
               GSETTINGS_BACKEND='memory', GIO_USE_VFS='local', GSK_RENDERER='cairo')
    os.environ['TACK_PROFILE_DIR'] = env['TACK_PROFILE_DIR']
    sessions, clips, checks = [], [], []
    duplicate = None
    def record(name, ok, detail=None):
        checks.append({'name': name, 'observed': bool(ok), 'detail': detail})
        (root / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
        if not ok:
            raise AssertionError(name)
    def start(name, arguments):
        s = Session(binary, root, name, arguments, env)
        sessions.append(s)
        return s
    def clipboard(data, target='UTF8_STRING'):
        p = subprocess.Popen(['xclip', '-selection', 'clipboard', '-in', '-quiet', '-target', target], stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        p.stdin.write(data)
        p.stdin.close()
        clips.append(p)
        time.sleep(.1)
    for i, size in enumerate([(180, 120), (90, 220), (320, 80), (160, 160)]):
        Image.new('RGB', size, (180 - i * 30, 50 + i * 40, 90 + i * 20)).save(root / f'image-{i}.png')
    board, target = root / 'board.tack', root / 'saved-as.tack'
    try:
        s = start('production', ['new', board])
        record('native new board is empty', len(saved_objects(board)['objects']) == 0)
        s.picker('ctrl+i', root / 'image-0.png')
        wait(lambda: 'modified' in s.title() and 'Import ' not in s.title(), 'single progressive import')
        first = s.save(board)
        record('native picker embedded import, save', len(first['objects']) == 1)
        drop(s.window, [root / 'image-1.png', root / 'image-2.png'])
        wait(lambda: 'modified' in s.title() and 'Import ' not in s.title(), 'multi drop')
        many = s.save(board)
        record('real XDND multi-file drop', len(many['objects']) == 3)
        clipboard((root / 'image-3.png').read_bytes(), 'image/png')
        s.key('ctrl+v')
        wait(lambda: 'modified' in s.title(), 'image clipboard')
        record('native clipboard PNG paste', len(s.save(board)['objects']) == 4)
        s.key('ctrl+comma', 'Down', 'Down', 'Return', 'Escape')
        wait(lambda: (root / 'profile/preferences.json').exists(), 'preferences publication')
        record('native linked default preference', not json.loads((root / 'profile/preferences.json').read_text())['embedded_import'])
        clipboard((root / 'image-1.png').as_uri().encode())
        s.key('ctrl+v')
        wait(lambda: 'modified' in s.title(), 'path clipboard')
        record('native clipboard file URI paste', len(s.save(board)['objects']) == 5)
        # Note drafts survive helper focus; paste uses the captured edit ticket.
        s.key('t')
        command('xdotool', 'mousemove', '--window', s.window, '30', '30')
        command('xdotool', 'click', '1')
        clipboard('Accent café\nReview refs'.encode())
        s.key('ctrl+v')
        time.sleep(.4)
        s.key('ctrl+Return')
        note = s.save(board)
        record('native clipboard UTF8 text into note', any(o.get('text') == 'Accent café\nReview refs' for o in note['objects']))
        clipboard(b'x' * (65536 + 1))
        s.key('ctrl+v')
        time.sleep(.4)
        s.shot('oversized-clipboard-error')
        record('oversized clipboard preserves saved authority', saved_objects(board) == note)
        s.key('Escape')
        drop(s.window, [root / 'image-0.png'] * 128)
        s.key('t')
        command('xdotool', 'mousemove', '--window', s.window, '500', '240')
        command('xdotool', 'click', '1')
        s.text('Note while import waits')
        time.sleep(.5)
        s.key('ctrl+Return')
        wait(lambda: 'Import ' not in s.title(), 'remaining imports resume after note commit', seconds=15)
        note = s.save(board)
        record('pending multi-import preserves note draft and resumes after commit', any(o.get('text') == 'Note while import waits' for o in note['objects']) and sum(o['kind'] == 'image' for o in note['objects']) == 133)
        # Search an action, add an extra binding, then reset it.
        s.key('ctrl+comma', *(['Down'] * 6), 'Return')
        s.text('NewBoard')
        s.key('Return', 'F9')
        wait(lambda: any(b['action'] == 'NewBoard' and 'F9' in str(b['control']) for b in json.loads((root / 'profile/preferences.json').read_text())['keymap']), 'keymap extra binding')
        record('native keymap search and second keyboard binding', True)
        s.shot('keymap')
        s.key('F5', 'Escape')
        wait(lambda: not any('F9' in str(b['control']) for b in json.loads((root / 'profile/preferences.json').read_text())['keymap']), 'reset action')
        record('native reset action restores defaults', True)
        s.key('ctrl+comma', *(['Down'] * 8), 'Return')
        # Export action opened the picker; fill it directly.
        dialog = wait(s.picker_window, 'owned export picker')
        command('xdotool', 'windowfocus', '--sync', dialog)
        s.key('ctrl+l', 'ctrl+a')
        s.text(str(root / 'keymap.json'))
        s.key('Return')
        time.sleep(.2)
        if not (root / 'keymap.json').exists():
            s.key('Return')
        wait(lambda: (root / 'keymap.json').exists(), 'keymap export')
        s.focus()
        record('readable native keymap export', json.loads((root / 'keymap.json').read_text())['version'] == 1)
        s.key('Escape')
        # Resize and focus cycle preserves work; no WM minimize claim.
        command('xdotool', 'windowsize', s.window, '820', '560')
        command('xdotool', 'windowunmap', s.window)
        time.sleep(.2)
        command('xdotool', 'windowmap', s.window)
        s.focus()
        s.shot('resized')
        record('native resize/unmap/restore preserves authority', s.save(board) == note)
        s.picker('ctrl+shift+s', target)
        wait(lambda: target.exists() and 'saved-as' in s.title(), 'SaveAs new identity path')
        record('native SaveAs retains document ID and geometry', saved_objects(target) == note)
        # Same-path owner refusal, independent distinct board succeeds.
        duplicate = subprocess.Popen([str(binary), 'open', str(target)], env=env, stdout=(root / 'duplicate.log').open('w'), stderr=subprocess.STDOUT)
        def duplicate_window():
            result = subprocess.run(['xdotool', 'search', '--onlyvisible', '--pid', str(duplicate.pid)], capture_output=True, text=True, timeout=3)
            return result.stdout.splitlines()[-1] if result.stdout.strip() else None
        duplicate_id = wait(duplicate_window, 'duplicate error window')
        wait(lambda: 'already open for editing' in command('xdotool', 'getwindowname', duplicate_id), 'explicit ownership refusal')
        record('same-file second native editor refused', True)
        close_owned_window(duplicate_id)
        duplicate.wait(timeout=8)
        other = start('separate', ['new', root / 'separate.tack'])
        s.focus()
        time.sleep(2)
        before = [observe(x.process.pid) for x in (s, other)]
        time.sleep(5)
        after = [observe(x.process.pid) for x in (s, other)]
        costs = []
        for a, b in zip(before, after):
            ticks = sum(t['ticks'] - a['tasks'][tid]['ticks'] for tid, t in b['tasks'].items() if tid in a['tasks'])
            costs.append({'rss_bytes': b['rss_bytes'], 'threads': len(b['tasks']), 'cpu_ticks_5s': ticks, 'io_delta': {k: b['io'][k] - a['io'][k] for k in a['io']}})
        record('distinct native boards idle independently', all(c['cpu_ticks_5s'] <= 5 and c['io_delta']['write_bytes'] == 0 for c in costs), costs)
        other.close()
        s.focus()
        # New draft commits at close; Cancel leaves the committed edit in place.
        s.key('t')
        command('xdotool', 'mousemove', '--window', s.window, '500', '50')
        command('xdotool', 'click', '1')
        s.text('Crash recovery note')
        s.key('ctrl+Return')
        normal_hash = digest(target)
        recovery = root / '.saved-as.tack.tack-recovery'
        wait(lambda: (recovery / 'state.meta').exists(), 'debounced recovery', seconds=12)
        record('recovery leaves authoritative normal file byte-identical', digest(target) == normal_hash)
        s.kill()
        restored = start('restore', ['open', target])
        restored.shot('decision')
        restored.key('Return')
        wait(lambda: 'modified' in restored.title(), 'explicit recovery restore')
        recovered = restored.save(target)
        record('native crash restore includes new note and normal Save', any(o.get('text') == 'Crash recovery note' for o in recovered['objects']))
        restored.close()
        reopen = start('reopen', ['open', target])
        record('close/reopen retains saved recovered document', saved_objects(target) == recovered)
        reopen.close()
        # Clean Untitled retirement must never follow an external alias.
        external = root / 'separate.tack'
        external_hash = digest(external)
        alias = root / 'profile/untitled-slot-1.tack'
        alias.symlink_to(external)
        seed = start('untitled-symlink', [])
        record('Untitled cleanup skips symlink to external empty board', alias.is_symlink() and external.exists() and digest(external) == external_hash and (root / 'profile/untitled-slot-2.tack').exists())
        seed.close()
        record('clean Untitled retires own seed only', alias.is_symlink() and external.exists() and not (root / 'profile/untitled-slot-2.tack').exists())
    except Exception:
        for s in sessions:
            if s.process.poll() is None:
                try:
                    (root / f'{s.name}-failure-title.txt').write_text(s.title())
                    s.shot('failure')
                except Exception:
                    pass
        raise
    finally:
        for s in sessions:
            s.kill()
        if duplicate is not None and duplicate.poll() is None:
            duplicate.kill()
            duplicate.wait(timeout=3)
        for clip in clips:
            if clip.poll() is None:
                clip.terminate()
                clip.wait(timeout=3)
    (root / 'provenance.json').write_text(json.dumps({'binary_sha256': digest(binary), 'harness_sha256': digest(__file__), 'drop_harness_sha256': digest(Path(__file__).with_name('x11_drop.py')), 'display': os.environ['DISPLAY'], 'checks': len(checks), 'input': 'owned generated files; real X11 keys/pointer, zenity, xclip and XDND; no artist feel or Windows runtime claim'}, indent=2) + '\n')


if __name__ == '__main__':
    main()
