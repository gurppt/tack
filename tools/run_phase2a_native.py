#!/usr/bin/env python3
"""Owned isolated X11: three native clients, semantic edits, refusal and rejoin.

Run only after the current client/server build, with no concurrent benchmark or
compiler. This starts its own server and generated small files. It never touches
an existing server, user board, desktop :0 or another process's windows.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import struct
import subprocess
import time
import uuid

from PIL import Image
from run_image_interaction import digest
from run_local_production import Session, wait
from run_native_image_checks import command
from run_phase1l_local import drag, pick_existing_file
from x11_drop import drop


def send_message(connection, message):
    payload = json.dumps(message, separators=(',', ':')).encode()
    connection.sendall(b'TLAN' + struct.pack('>HI', 2, len(payload)) + payload)


def read_exact(connection, length):
    parts, remaining = [], length
    while remaining:
        part = connection.recv(remaining)
        if not part:
            raise EOFError('owned server closed a truncated frame')
        parts.append(part)
        remaining -= len(part)
    return b''.join(parts)


def read_message(connection):
    header = read_exact(connection, 10)
    magic, major, length = struct.unpack('>4sHI', header)
    if magic != b'TLAN' or major != 2 or not 0 < length <= 64 * 1024 * 1024:
        raise AssertionError('invalid server frame')
    return json.loads(read_exact(connection, length))


def snapshot(address, board):
    host, port = address.rsplit(':', 1)
    with socket.create_connection((host, int(port)), timeout=3) as connection:
        connection.settimeout(5)
        send_message(connection, {'type': 'hello', 'board': board,
                                  'client': uuid.uuid4().hex, 'revision': 0})
        result = read_message(connection)
        if result['type'] != 'snapshot':
            raise AssertionError(result)
        return result


def native_revision(session):
    title = session.title()
    match = re.search(r' · rev (\d+) ·', title)
    return int(match.group(1)) if match and ' · Connected ·' in title else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('TACK_NATIVE_NO_WM') != '1' or os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        parser.error('requires isolated X11 DISPLAY and TACK_NATIVE_NO_WM=1')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary, server_binary = args.binary.resolve(), args.server.resolve()
    env = dict(os.environ, TACK_TEST_WINDOW_SIZE='800x600', WINIT_X11_SCALE_FACTOR='1',
               XDG_DATA_HOME=str(root/'xdg-data'), XDG_CONFIG_HOME=str(root/'xdg-config'),
               GSETTINGS_BACKEND='memory', GIO_USE_VFS='local', GSK_RENDERER='cairo')
    sessions, checks, server_logs = [], [], []
    server = None
    server_root = root/'server-data'
    started = time.monotonic()

    def record(name, observed, detail=None):
        checks.append({'name': name, 'observed': bool(observed), 'detail': detail})
        (root/'checks.json').write_text(json.dumps(checks, indent=2)+'\n')
        if not observed:
            raise AssertionError(name)

    def cli(*arguments):
        result = subprocess.run([str(binary), *map(str, arguments)], env=env,
                                capture_output=True, text=True, timeout=30)
        (root/f'cli-{len(list(root.glob("cli-*.log")))}.log').write_text(result.stdout+result.stderr)
        if result.returncode:
            raise AssertionError(f'owned CLI {arguments[0]}: {result.stderr}')
        return json.loads(result.stdout)

    def launch_server(address):
        log_path = root/f'server-{len(server_logs)}.log'
        log = log_path.open('w')
        server_logs.append(log)
        process = subprocess.Popen([str(server_binary), '--listen', address,
                                    '--root', str(server_root), '--asset-quota', str(32*1024*1024)],
                                   stdout=log, stderr=subprocess.STDOUT, env=env)
        def listening():
            if process.poll() is not None:
                raise AssertionError(f'owned server exited {process.returncode}: {log_path.read_text()}')
            match = re.search(r'listen=(\S+)', log_path.read_text())
            return match.group(1) if match else None
        return process, wait(listening, 'owned server listening', seconds=8)

    def start(name, arguments):
        local_env = dict(env, TACK_PROFILE_DIR=str(root/f'{name}-profile'))
        session = Session(binary, root, name, arguments, local_env)
        original_focus = session.focus
        def owned_focus():
            command('xdotool', 'windowraise', session.window)
            original_focus()
        session.focus = owned_focus
        original_shot = session.shot
        def owned_shot(label):
            session.focus()
            original_shot(label)
        session.shot = owned_shot
        sessions.append(session)
        return session

    def require_image_supply(session, phase):
        # Title/authority convergence alone cannot prove that CAS originals reach
        # the native renderer. Capture the actual owned drawable and count the
        # fixture's red pixels, which differ from background, placeholder and UI.
        capture = root/f'{session.name}-{phase}-supply.png'
        latest = {}
        def supplied():
            session.focus()
            title = session.title()
            if '0 missing / 0 changed sources' not in title or 'shared source unavailable' in title.lower():
                latest['title'] = title
                return False
            subprocess.run(['import', '-window', session.window, str(capture)],
                           check=True, timeout=4)
            with Image.open(capture) as frame:
                pixels = frame.convert('RGB')
                red = sum(1 for r,g,b in pixels.getdata() if r > 145 and g < 110 and b < 120)
            latest.update(title=title, original_red_pixels=red, capture=capture.name)
            return red >= 10000
        try:
            wait(supplied, f'{session.name} {phase} actual native original supply', seconds=20)
        except AssertionError as error:
            raise AssertionError(f'{error}: {latest}') from error
        record(f'{session.name} {phase} displays verified originals without unavailable source',
               True, latest)

    def await_revision(shared, revision, description):
        wait(lambda: all(native_revision(session) == revision for session in shared),
             description, seconds=15)
        return snapshot(address, board_id)

    def changed_by(session, shared, action, name, expected_objects):
        before = snapshot(address, board_id)['revision']
        session.focus()
        action()
        state = await_revision(shared, before+1, name)
        record(name, state['revision'] == before+1 and
               state['document']['counts'][2] == expected_objects,
               {'revision': state['revision'], 'objects': state['document']['counts'][2],
                'native_titles': [s.title() for s in shared]})
        return state

    image = root/'source.png'
    Image.new('RGB', (400, 200), (190, 70, 40)).save(image)
    local_board = root/'local-source.tack'
    cli('create', local_board, '--linked', image)
    board_id = cli('inspect', local_board)['document_id']
    input_hash = digest(local_board)
    try:
        server, address = launch_server('127.0.0.1:0')
        published = cli('publish', local_board, address)
        initial = snapshot(address, board_id)
        record('publish retains stable board identity and source local file',
               initial['board'] == board_id and initial['document']['counts'] == [1, 1, 1]
               and digest(local_board) == input_hash, published)
        # Linked local original may remain in this local board; LAN authority never uses its path.
        local = start('local-alongside', ['open', local_board])
        shared = [start(f'client-{letter}', ['join', address, board_id]) for letter in 'ABC']
        await_revision(shared, 0, 'three real native clients joined authoritative board')
        record('three simultaneous native clients and an independent local board',
               len({s.process.pid for s in shared}) == 3 and
               all(s.process.poll() is None for s in shared+[local]),
               {'shared_pids': [s.process.pid for s in shared],
                'windows': [s.window for s in shared], 'local_pid': local.process.pid,
                'display': os.environ['DISPLAY']})
        for session in shared:
            require_image_supply(session, 'joined')

        def note():
            shared[0].key('t')
            drag(shared[0], (70, 40), (310, 130))
            shared[0].text('Phase 2A native A')
            shared[0].key('ctrl+Return', 'v')
        changed_by(shared[0], shared, note, 'A note appears authoritatively in B and C', 2)

        def rectangle():
            shared[1].key('r')
            drag(shared[1], (480, 460), (710, 550))
            shared[1].key('v')
        changed_by(shared[1], shared, rectangle, 'B rectangle appears authoritatively in A and C', 3)
        changed_by(shared[1], shared, lambda: shared[1].key('ctrl+z'),
                   'B shared undo is a new accepted inverse', 2)
        changed_by(shared[1], shared, lambda: shared[1].key('ctrl+y'),
                   'B shared redo restores authoritative rectangle', 3)
        revision = snapshot(address, board_id)['revision']
        shared[0].focus()
        shared[0].key('ctrl+z')
        wait(lambda: 'refused' in shared[0].title().lower(), 'A conflicting undo visible refusal')
        record('A undo cannot overwrite B accepted work',
               snapshot(address, board_id)['revision'] == revision and
               all(native_revision(session) == revision for session in shared), shared[0].title())
        shared[0].key('Escape')

        def move_image():
            shared[2].key('v')
            drag(shared[2], (400, 300), (440, 325))
        changed_by(shared[2], shared, move_image, 'native image drag is one durable semantic operation', 3)

        def edit_note():
            shared[0].key('v')
            command('xdotool', 'mousemove', '--window', shared[0].window, '170', '85')
            command('xdotool', 'click', '1')
            shared[0].key('F2', 'ctrl+a')
            shared[0].text('Edited by native A')
            shared[0].key('ctrl+Return')
        final = changed_by(shared[0], shared, edit_note, 'native note text edit converges in three clients', 3)
        for session in shared:
            session.shot('before-restart')

        server.terminate()
        server.wait(timeout=8)
        wait(lambda: all(native_revision(session) is None for session in shared),
             'LAN loss is visible in all native clients', seconds=12)
        shared[2].focus()
        shared[2].key('r')
        drag(shared[2], (40, 460), (150, 540))
        shared[2].key('Escape', 'Escape', 'v')
        local.focus()
        local.key('ctrl+a', 'Escape')
        record('disconnected shared clients stay alive and local board responds',
               all(s.process.poll() is None for s in shared+[local]) and
               digest(local_board) == input_hash,
               {'native_titles': [s.title() for s in shared], 'local_title': local.title()})
        server, recovered_address = launch_server(address)
        record('server restart retains original listener and durable authority',
               recovered_address == address and snapshot(address, board_id)['revision'] == final['revision'])
        for session in shared:
            session.focus()
            session.key('F5')
        reconciled = await_revision(shared, final['revision'], 'three native clients explicitly rejoin after server restart')
        record('offline attempted edit was not merged or replayed',
               reconciled['document'] == final['document'] and reconciled['revision'] == final['revision'])
        def after_rejoin():
            shared[2].key('r')
            drag(shared[2], (40, 460), (160, 540))
            shared[2].key('v')
        final = changed_by(shared[2], shared, after_rejoin, 'native edit works after deterministic rejoin', 4)
        replacement_image = root/'replacement.png'
        replacement_image_2 = root/'replacement-2.png'
        Image.new('RGB', (400, 200), (40, 130, 205)).save(replacement_image)
        Image.new('RGB', (400, 200), (140, 80, 160)).save(replacement_image_2)
        def relink(path):
            shared[2].key('v')
            command('xdotool', 'mousemove', '--window', shared[2].window, '440', '325')
            command('xdotool', 'click', '1')
            pick_existing_file(shared[2], 'ctrl+shift+r', path)
        final = changed_by(shared[2], shared, lambda: relink(replacement_image),
                           'native source replacement converges through validated CAS binding', 4)
        record('first relink uses persisted source revision floor',
               max(binding['revision'] for binding in final['sources']) == 2 and
               final['source_high_water'] == 2)
        final = changed_by(shared[2], shared, lambda: shared[2].key('ctrl+z'),
                           'shared source undo restores original binding without reusing revision', 4)
        record('undo retains high water above restored live revision',
               final['sources'][0]['revision'] == 1 and final['source_high_water'] == 2)
        shared[2].focus()
        shared[2].key('F5')
        time.sleep(.4)
        await_revision(shared, final['revision'], 'native source editor rejoins after undo')
        final = changed_by(shared[2], shared, lambda: relink(replacement_image_2),
                           'relink after undo and rejoin advances to a fresh source revision', 4)
        record('rejoin preserved source high water independently from document undo',
               final['sources'][0]['revision'] == 3 and final['source_high_water'] == 3)

        # Two queued imports from one native client must not stale-refuse one another.
        before = final['revision']
        stored_before = sorted(p.name for p in (server_root/'assets').iterdir()
                               if len(p.name) == 64)
        duplicate_path = root/'same-content-different-path.png'
        duplicate_path.write_bytes(image.read_bytes())
        shared[2].focus()
        drop(shared[2].window, [image, duplicate_path])
        wait(lambda: 'Import ' not in shared[2].title() and
             all(native_revision(session) == before+2 for session in shared),
             'two-file native drop accepted without own-client stale rejection', seconds=20)
        final = snapshot(address, board_id)
        stored_after = sorted(p.name for p in (server_root/'assets').iterdir()
                              if len(p.name) == 64)
        record('two-file native import serializes own operations and deduplicates originals',
               final['document']['counts'] == [3, 3, 6] and stored_before == stored_after,
               {'revision': final['revision'], 'counts': final['document']['counts'],
                'cas_files_before': len(stored_before), 'cas_files_after': len(stored_after)})
        before = final['revision']
        shared[2].focus()
        drop(shared[2].window, [image, image])
        wait(lambda: 'Import ' not in shared[2].title() and
             all(native_revision(session) == before+2 for session in shared),
             'same-path duplicate native imports both accepted', seconds=20)
        final = snapshot(address, board_id)
        record('same-path duplicate imports share one source and retain two objects',
               final['document']['counts'] == [4, 4, 8] and
               sorted(p.name for p in (server_root/'assets').iterdir() if len(p.name) == 64) == stored_before,
               {'revision': final['revision'], 'counts': final['document']['counts']})

        # Switching native focus would cancel the drag independently. Keep C's
        # actual button grab and send one semantic edit from a scoped protocol peer.
        shared[2].focus()
        shared[2].key('Escape', 'v')
        command('xdotool', 'mousemove', '--window', shared[2].window, '440', '325')
        command('xdotool', 'mousedown', '1')
        try:
            command('xdotool', 'mousemove', '--window', shared[2].window, '485', '350')
            time.sleep(.15)
            host, port = address.rsplit(':', 1)
            with socket.create_connection((host, int(port)), timeout=3) as peer:
                peer.settimeout(5)
                client = uuid.uuid4().hex
                send_message(peer, {'type': 'hello', 'board': board_id, 'client': client,
                                    'revision': final['revision']})
                read_message(peer)
                send_message(peer, {'type': 'edit', 'operation': uuid.uuid4().hex,
                    'base': final['revision'], 'sources': [],
                    'command': {'kind': 'add_object', 'index': 8,
                        'object': {'kind': 'annotation', 'id': uuid.uuid4().hex,
                            'annotation': {'kind': 'rect'},
                            'style': {'stroke': [255, 198, 82, 255], 'fill': None,
                                      'width': 3, 'opacity': 1},
                            'transform': {'center': [-300, -300], 'size': [40, 20],
                                          'rotation': 0, 'flips': [False, False]}}}})
                response = read_message(peer)
                record('scoped remote conflict operation accepted', response['type'] == 'accepted')
            final = await_revision(shared, final['revision']+1,
                                   'remote accepted edit reaches active native gesture')
            wait(lambda: 'active edit cancelled' in shared[2].title().lower(),
                 'remote edit cancellation is visible in native window')
        finally:
            command('xdotool', 'mouseup', '1')
        time.sleep(.35)
        record('remote semantic edit cancels an active native drag without committing stale geometry',
               snapshot(address, board_id)['revision'] == final['revision'] and
               final['document']['counts'][2] == 9,
               {'revision': final['revision'], 'title': shared[2].title(),
                'scope': 'C native pointer/button gesture, incoming operation from owned protocol peer'})
        authority_digest = hashlib.sha256(bytes.fromhex(final['document']['metadata'])).hexdigest()
        for session in shared:
            require_image_supply(session, 'converged')
        for session in shared:
            session.close()
        local.close()
        reports = [json.loads(session.report.read_text()) for session in shared]
        record('all three native final documents equal persisted authority',
               all(report['shared']['revision'] == final['revision'] and
                   report['shared']['canonical_sha256'] == authority_digest and
                   report['shared']['object_count'] == 9 for report in reports),
               {'revision': final['revision'], 'canonical_sha256': authority_digest,
                'clients': [report['shared'] for report in reports]})
        record('all three native clients supplied actual images and retain no unavailable sources',
               all(report.get('source_missing') == 0 and
                   report.get('source_unavailable') == 0 and
                   report.get('source_foreign') == 0 and
                   report.get('first_recognizable_ms') is not None and
                   report.get('frames') and
                   report['frames'][-1].get('recognizable', 0) > 0
                   for report in reports),
               [{'name': session.name, 'source_missing': report.get('source_missing'),
                 'source_unavailable': report.get('source_unavailable'),
                 'first_recognizable_ms': report.get('first_recognizable_ms'),
                 'last_frame_recognizable': report['frames'][-1].get('recognizable', 0)}
                for session,report in zip(shared,reports)])
        record('local board alongside shared clients had no shared backend and no disk mutation',
               'shared' not in json.loads(local.report.read_text()) and digest(local_board) == input_hash)
    except Exception:
        for session in sessions:
            if session.process.poll() is None:
                try:
                    session.shot('failure')
                    (root/f'{session.name}-failure-title.txt').write_text(session.title())
                except (OSError, subprocess.SubprocessError):
                    pass
        raise
    finally:
        for session in sessions:
            session.kill()
        if server is not None and server.poll() is None:
            server.terminate()
            server.wait(timeout=8)
        for log in server_logs:
            log.close()
    (root/'provenance.json').write_text(json.dumps({
        'client_sha256': digest(binary), 'server_sha256': digest(server_binary),
        'harness_sha256': digest(__file__), 'display': os.environ['DISPLAY'],
        'elapsed_seconds': time.monotonic()-started, 'checks': len(checks),
        'scope': 'actual Linux/X11 native keys and pointer; isolated owned server/files; no Windows or artist-feel claim',
    }, indent=2)+'\n')
    print(f'{len(checks)} Phase 2A native checks passed', flush=True)


if __name__ == '__main__':
    main()
