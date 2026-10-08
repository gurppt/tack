#!/usr/bin/env python3
"""Owned serial LAN/native receipts: traffic, idle, stress, restart and bounds."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import time
import threading
import uuid
from PIL import Image
from run_local_production import Session, wait
from run_image_interaction import digest, distribution
from run_native_image_checks import command
from run_phase1l_local import drag
from run_phase2a_native import read_message, send_message, snapshot, native_revision
from run_phase2a_local_regression import snapshot as process_snapshot, delta


class Peer:
    def __init__(self, address, board):
        host, port = address.rsplit(':', 1)
        self.socket = socket.create_connection((host, int(port)), timeout=5)
        self.socket.settimeout(10)
        self.sent = self.received = self.messages_sent = self.messages_received = 0
        self.client = uuid.uuid4().hex
        self.send({'type': 'hello', 'board': board, 'client': self.client, 'revision': 0})
        self.state = self.read()
        if self.state['type'] != 'snapshot': raise AssertionError(self.state)
    def send(self, message):
        payload = json.dumps(message, separators=(',', ':')).encode()
        self.sent += len(payload) + 10
        self.messages_sent += 1
        send_message(self.socket, message)
    def read(self):
        # Server JSON compact serialization; captured payload byte length is exact.
        import struct
        from run_phase2a_native import read_exact
        header = read_exact(self.socket, 10)
        magic, major, length = struct.unpack('>4sHI', header)
        if magic != b'TLAN' or major != 1 or length > 64*1024*1024: raise AssertionError('framing')
        payload = read_exact(self.socket, length)
        self.received += length + 10
        self.messages_received += 1
        return json.loads(payload)
    def counters(self):
        return {'bytes_sent': self.sent, 'bytes_received': self.received, 'messages_sent': self.messages_sent, 'messages_received': self.messages_received}
    def edit(self, revision, cmd):
        before = self.counters()
        started = time.monotonic()
        self.send({'type': 'edit', 'operation': uuid.uuid4().hex, 'base': revision, 'command': cmd, 'sources': []})
        accepted = self.read()
        if accepted['type'] != 'accepted': raise AssertionError(accepted)
        return accepted['revision'], {'wall_ms': (time.monotonic()-started)*1000,
            'traffic': {key: value-before[key] for key,value in self.counters().items()}, 'command': cmd['kind']}
    def close(self): self.socket.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0'): parser.error('isolated owned X11 required')
    root = args.output.resolve(); root.mkdir(parents=True, exist_ok=False)
    binary, server_binary = args.binary.resolve(), args.server.resolve()
    env = dict(os.environ, TACK_NATIVE_NO_WM='1', TACK_TEST_WINDOW_SIZE='800x600', WINIT_X11_SCALE_FACTOR='1')
    data = {'client_sha256': digest(binary), 'server_sha256': digest(server_binary), 'harness_sha256': digest(__file__),
        'fixture_sha256': digest(args.fixture), 'scope': 'Samehost RTX2060 X11 800x600 loopback trustedLAN protocol bytes (not IP packet overhead); no networkhardware bandwidth/Windows/physicalcold-I/O claim',
        'server_idle': [], 'client_idle': [], 'operations': [], 'stress': []}
    def publish(): (root/'summary.json').write_text(json.dumps(data, indent=2)+'\n')
    image = root/'image.png'; Image.new('RGB', (400,200), (100,130,190)).save(image)
    boards = []
    for name, counts in [('images1k', [1000,0,0]), ('shapes10k-notes100', [0,10000,100])]:
        path = root/f'{name}.tack'
        result = subprocess.run([str(args.fixture.resolve()), str(path), str(image), *map(str,counts)], capture_output=True, text=True, check=True, timeout=20)
        board = json.loads(result.stdout); board['path'] = path; boards.append(board)
    log_path = root/'server.log'; log = log_path.open('w')
    server = subprocess.Popen([str(server_binary), '--listen','127.0.0.1:0','--root',str(root/'server-data'),'--asset-quota',str(64*1024*1024)],stdout=log,stderr=subprocess.STDOUT,env=env)
    sessions = []; peers = []
    def listening():
        if server.poll() is not None: raise AssertionError(log_path.read_text())
        match = re.search(r'listen=(\S+)', log_path.read_text()); return match.group(1) if match else None
    try:
        address = wait(listening, 'owned server listen')
        for board in boards:
            result = subprocess.run([str(binary),'publish',str(board['path']),address],capture_output=True,text=True,check=True,timeout=20)
            data.setdefault('published',[]).append(json.loads(result.stdout))
        for count in range(4):
            if count:
                name = f'native-{count}'
                session = Session(binary, root, name, ['join',address,boards[0]['board']], dict(env,TACK_PROFILE_DIR=str(root/f'{name}-profile')))
                command('xdotool','windowraise',session.window)
                sessions.append(session)
                wait(lambda: native_revision(session) == 0, 'native 1k shared load')
            time.sleep(6)
            before_server = process_snapshot(server.pid)
            before_clients = [process_snapshot(session.process.pid) for session in sessions]
            started = time.monotonic(); time.sleep(3); seconds = time.monotonic()-started
            after_server = process_snapshot(server.pid)
            after_clients = [process_snapshot(session.process.pid) for session in sessions]
            data['server_idle'].append({'clients':count, **delta(before_server,after_server,seconds)})
            data['client_idle'].extend({'client_index':i+1,'simultaneous_clients':count, **delta(a,b,seconds)} for i,(a,b) in enumerate(zip(before_clients,after_clients)))
            publish()
        # Real native drag and note traffic are measured from final client counters;
        # a separate peer isolates exact control operation receipts below.
        # All three diagnostic probes must finish before injecting traffic.
        time.sleep(8)
        board = boards[0]['board']; peer = Peer(address,board); peers.append(peer)
        revision = peer.state['revision']
        for amount in [1,10]:
            edits = [{'kind':'set_transform','object':f'{index+1:032x}',
                      'transform':{'center':[index*120+15.,15.], 'size':[100.,60.], 'rotation':0,'flips':[False,False]}} for index in range(amount)]
            revision, receipt = peer.edit(revision, {'kind':'batch','edits':edits})
            data['operations'].append({'name':f'{amount}-object-completed-manipulation',**receipt})
            wait(lambda: all(native_revision(session)==revision for session in sessions), 'native authoritative operation convergence')
        interaction_start = revision
        interaction_errors = []
        interaction_receipts = []
        def synchronize():
            nonlocal revision
            try:
                for index in range(40):
                    revision, receipt = peer.edit(revision, {'kind':'set_transform','object':f'{100:032x}',
                        'transform':{'center':[100.+index,50.], 'size':[100.,60.], 'rotation':0,'flips':[False,False]}})
                    interaction_receipts.append(receipt['wall_ms'])
                    time.sleep(.01)
            except Exception as error:
                interaction_errors.append(repr(error))
        active = sessions[-1]
        command('xdotool','windowraise',active.window); active.focus()
        command('xdotool','mousemove','--window',active.window,'400','300')
        started = time.monotonic()
        worker = threading.Thread(target=synchronize); worker.start()
        command('xdotool','mousedown','2')
        for index in range(20):
            command('xdotool','mousemove','--window',active.window,str(400+index*2),'300')
            if index % 2 == 0: command('xdotool','click','4' if index % 4 == 0 else '5')
            time.sleep(.01)
        command('xdotool','mouseup','2')
        worker.join(timeout=15)
        if worker.is_alive() or interaction_errors: raise AssertionError(interaction_errors or 'sync deadline')
        wait(lambda: all(native_revision(session)==revision for session in sessions), 'native active-sync convergence')
        data['active_sync'] = {'accepted_operations':revision-interaction_start,'wall_seconds':time.monotonic()-started,
            'control_latency_ms':distribution(interaction_receipts),'native_pan_steps':20,'native_wheel_events':10,
            'latency_scope':'Report event/wheel callback processing is a proxy, not end-to-end physical input latency; includes other window callbacks outside this interval.'}
        peer.close(); peers.remove(peer)
        stress_board = boards[1]['board']; stress = Peer(address,stress_board); peers.append(stress)
        revision = stress.state['revision']; timings = []
        before = process_snapshot(server.pid); started = time.monotonic()
        for index in range(100):
            cmd = {'kind':'set_transform','object':f'{index+1:032x}', 'transform':{'center':[index*120.+1.,1.], 'size':[100.,60.], 'rotation':0,'flips':[False,False]}}
            revision, receipt = stress.edit(revision,cmd); timings.append(receipt['wall_ms'])
        elapsed = time.monotonic()-started; after = process_snapshot(server.pid)
        data['stress'].append({'name':'100-edits-on-10k-shapes-100-notes', 'counts':stress.state['document']['counts'], 'operation_latency_ms':distribution(timings), 'wall_seconds':elapsed, 'server':delta(before,after,elapsed)})
        cmd = {'kind':'set_text','object':f'{10001:032x}', 'text':{'value':'Phase2A shared note café 猫','font_size':12,'alignment':0}}
        revision, receipt = stress.edit(revision,cmd); data['operations'].append({'name':'note-edit',**receipt})
        before = stress.counters(); started = time.monotonic()
        stress.send({'type':'hello','board':stress_board,'client':stress.client,'revision':0})
        current = stress.read()
        data['reconnect'] = {'known_revision_gap':revision, 'strategy':'validated full metadata snapshot; no durable replay', 'wall_ms':(time.monotonic()-started)*1000,
            'traffic':{key:value-before[key] for key,value in stress.counters().items()}, 'revision':current['revision']}
        if current['revision'] != revision: raise AssertionError('reconnect revision')
        stress.close(); peers.remove(stress)
        stress_session = Session(binary, root, 'stress-native', ['join',address,stress_board], dict(env,TACK_PROFILE_DIR=str(root/'stress-profile')))
        sessions.append(stress_session)
        wait(lambda: native_revision(stress_session) == revision, 'native 10k shapes 100 notes revision')
        command('xdotool','windowraise',stress_session.window);stress_session.focus();stress_session.key('F5')
        wait(lambda: native_revision(stress_session) == revision and 'Connected' in stress_session.title(), 'native rejoin after 101 edits')
        stress_session.close();sessions.remove(stress_session)
        stress_report = json.loads(stress_session.report.read_text())
        data['stress_native'] = {'revision':revision,'annotations':stress_report['annotations'],'rejoined':True,
            'canonical_sha256':stress_report['shared']['canonical_sha256'],'report_sha256':digest(stress_session.report)}
        if stress_report['annotations'] != 10100:raise AssertionError('native stress metadata count')
        # Actual bounded CAS upload throughput, independent from tiny fixture supply.
        from io import BytesIO
        encoded = BytesIO(); Image.effect_noise((2500,2000),100).save(encoded, format='PNG'); payload = encoded.getvalue()
        content = hashlib.sha256(payload).hexdigest(); host,port=address.rsplit(':',1)
        with socket.create_connection((host,int(port)),timeout=5) as upload:
            upload.settimeout(5); started=time.monotonic(); total=0; messages=0
            def request(message):
                nonlocal total,messages
                total+=len(json.dumps(message,separators=(',',':')).encode())+10;messages+=1
                send_message(upload,message);return read_message(upload)
            result=request({'type':'asset_begin','hash':content,'size':len(payload)})
            if result['type'] != 'asset_status' or result['present']:raise AssertionError(result)
            for offset in range(0,len(payload),64*1024):
                result=request({'type':'asset_chunk','hash':content,'offset':offset,'bytes':payload[offset:offset+64*1024].hex()})
                if result['type'] != 'asset_progress':raise AssertionError(result)
            result=request({'type':'asset_commit','hash':content});seconds=time.monotonic()-started
            if result['type']!='asset_ready':raise AssertionError(result)
            dedupe=request({'type':'asset_begin','hash':content,'size':len(payload)})
            data['upload']={'payload_bytes':len(payload),'seconds':seconds,'MiB_per_second':len(payload)/1024**2/seconds,'wire_bytes_sent_including_dedupe':total,'messages_including_dedupe':messages,'dedupe_present':dedupe.get('present') is True}
        for session in sessions:
            command('xdotool','windowraise',session.window);session.focus()
            command('xdotool','mousemove','--window',session.window,'400','300')
            command('xdotool','mousedown','2')
            for index in range(20):command('xdotool','mousemove','--window',session.window,str(400+index*2),'300');time.sleep(.01)
            command('xdotool','mouseup','2')
            client_hwm=next(int(line.split()[1])*1024 for line in Path(f'/proc/{session.process.pid}/status').read_text().splitlines() if line.startswith('VmHWM:'))
            session.close()
            report=json.loads(session.report.read_text())
            probes=report['shared']['traffic_probes']
            if len(probes)!=2:raise AssertionError('two complete diagnostic idle probes')
            diff={key:probes[1][key]-probes[0][key] for key in ('bytes_sent','bytes_received','messages_sent','messages_received')}
            seconds=probes[1]['elapsed_seconds']-probes[0]['elapsed_seconds']
            data.setdefault('native_clients',[]).append({'name':session.name,'shared':report['shared'], 'idle_network_delta':diff,'idle_network_seconds':seconds,
                'event_ms':distribution(report['event_samples_ms']), 'wheel_ms':distribution(report['wheel_samples_ms']), 'first_frame_ms':report['first_frame_ms'], 'rss_peak':client_hwm})
            if any(diff.values()):raise AssertionError('idle network traffic')
            if report['errors'] or report['source_missing'] or report['source_unavailable']:raise AssertionError('valid fixture original unavailable')
        # Remove only the known generated server-owned fixture original. A fresh
        # private-cache client must remain usable and visibly report missing data.
        tiny_hash = hashlib.sha256(image.read_bytes()).hexdigest()
        (root/'server-data'/'assets'/tiny_hash).unlink()
        missing = Session(binary, root, 'missing-native', ['join',address,board], dict(env,TACK_PROFILE_DIR=str(root/'missing-profile')))
        sessions.append(missing)
        wait(lambda: 'Shared source unavailable' in missing.title(), 'native missing shared original visible')
        time.sleep(2)
        before = process_snapshot(missing.process.pid); started = time.monotonic()
        time.sleep(3)
        after = process_snapshot(missing.process.pid)
        missing_idle = delta(before,after,time.monotonic()-started)
        missing_idle_start_ms = (started-missing.started)*1000
        missing_idle_end_ms = (time.monotonic()-missing.started)*1000
        if missing_idle['ticks'] > 1:raise AssertionError('missing source caused idle retry work')
        missing.close()
        missing_report = json.loads(missing.report.read_text())
        missing_idle['frames'] = sum(missing_idle_start_ms <= frame['elapsed_ms'] <= missing_idle_end_ms for frame in missing_report['frames'])
        if missing_idle['frames'] > 2:raise AssertionError('missing source caused continual redraw')
        data['missing_asset'] = {'explicit_error':True,'revision':missing_report['shared']['revision'],
            'objects':missing_report['shared']['object_count'],'idle':missing_idle,'report_sha256':digest(missing.report)}
        publish()
    finally:
        for peer in peers:peer.close()
        for session in sessions:session.kill()
        if server.poll() is None:server.terminate();server.wait(timeout=8)
        log.close()
    print(root/'summary.json',flush=True)


if __name__=='__main__':main()
