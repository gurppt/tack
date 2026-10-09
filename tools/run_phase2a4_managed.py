#!/usr/bin/env python3
"""Bounded owned managed-server EOF/checkpoint and persistent authority witness."""
import argparse
import json
from pathlib import Path
import re
import shutil
import socket
import subprocess
import uuid
from run_local_production import wait
from run_native_annotation_checks import saved_objects
from run_image_interaction import digest
from run_phase2a_native import send_message, read_message, snapshot


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ['binary', 'server', 'native', 'output']:
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    root = a.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    original = a.native.resolve() / 'artist-shared.tack'
    descriptor = json.loads(Path(str(original) + '.sharing.json').read_text())
    source = a.native.resolve() / 'artist-a-profile/hosted' / descriptor['board']
    size = sum(f.stat().st_size for f in source.rglob('*') if f.is_file())
    if size > 8*1024*1024 or original.stat().st_size > 8*1024*1024:
        raise AssertionError('Use only small owned native fixture')
    authority = root / 'authority'
    shutil.copytree(source, authority)
    offline = root / 'offline.tack'
    shutil.copy2(original, offline)
    checks, children, logs = [], [], []

    def record(name, ok, detail=None):
        checks.append(dict(name=name, observed=bool(ok), detail=detail))
        (root / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
        if not ok:
            raise AssertionError(name)

    def start(name):
        logpath = root / f'{name}.log'
        log = logpath.open('w')
        logs.append(log)
        ready = root / f'{name}.ready'
        child = subprocess.Popen([str(a.server.resolve()), '--root', str(authority),
            '--listen', '127.0.0.1:0', '--managed-ready', str(ready),
            '--managed-snapshot', str(offline), '--managed-board', descriptor['board']],
            stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT)
        children.append(child)
        wait(lambda: ready.exists() and ready.read_text() == 'ready', 'managed readiness')
        address = re.search(r'listen=(\S+)', logpath.read_text())[1]
        return child, address

    def end(child):
        child.stdin.close()  # The same EOF observed after host-process death.
        child.wait(timeout=15)
        return child.returncode == 0 and not Path(f'/proc/{child.pid}').exists()

    try:
        child, address = start('first')
        current = snapshot(address, descriptor['board'])
        before = digest(offline)
        object_id = saved_objects(offline)['objects'][0]['id']
        host, port = address.rsplit(':', 1)
        with socket.create_connection((host, int(port)), timeout=3) as peer:
            peer.settimeout(5)
            send_message(peer, dict(type='hello', board=descriptor['board'], client=uuid.uuid4().hex, revision=current['revision']))
            read_message(peer)
            send_message(peer, dict(type='edit', operation=uuid.uuid4().hex, base=current['revision'], sources=[],
                command=dict(kind='set_opacity', object=f'{object_id:032x}', opacity=.5)))
            record('managed server accepts existing semantic edit', read_message(peer)['type'] == 'accepted')
        accepted = snapshot(address, descriptor['board'])
        record('online authority advances while offline file remains snapshot',
               accepted['revision'] == current['revision']+1 and digest(offline) == before)
        record('stdin EOF checkpoints and exits with child reaped', end(child))
        inspected = subprocess.run([str(a.binary.resolve()), 'inspect', str(offline)],
                                   capture_output=True, text=True, check=True, timeout=10)
        record('EOF exports checked changed snapshot with stable ID',
               digest(offline) != before and json.loads(inspected.stdout)['document_id'] == descriptor['board'])
        reopened, address2 = start('reopened')
        restored = snapshot(address2, descriptor['board'])
        record('restart reuses accepted authority instead of publishing snapshot',
               restored['revision'] == accepted['revision'] and restored['document'] == accepted['document'])
        record('second EOF closes cleanly without hidden server', end(reopened))
        (root / 'receipt.json').write_text(json.dumps(dict(server_sha256=digest(a.server),
            harness_sha256=digest(__file__), copied_fixture_bytes=size+original.stat().st_size,
            scope='Direct EOF lifetime simulation, not a physical machine crash', checks=checks), indent=2)+'\n')
    finally:
        for child in children:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=4)
        for log in logs:
            log.close()
    print(root / 'receipt.json')


if __name__ == '__main__':
    main()
