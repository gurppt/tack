#!/usr/bin/env python3
"""Bounded owned-server lease capacity/RSS measurement; no renderer or image corpus."""
import argparse
import json
from pathlib import Path
import re
import socket
import struct
import subprocess
import time
import uuid

from run_local_production import wait
from run_phase2a_native import send_message, read_message
from run_phase2a_local_regression import snapshot, delta
from run_image_interaction import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('server', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    sockets = []
    checks = []
    board = uuid.uuid4().hex
    ids = list(range(1, 1026))
    # Canonical schema-2 fixture: document identity, Frame records, object order,
    # empty group table. The real server decodes and validates it before publish.
    metadata = bytearray(int(board, 16).to_bytes(16, 'little'))
    for identity in ids:
        metadata.extend(struct.pack('<HH', 1, 2))
        metadata.extend(identity.to_bytes(16, 'little'))
        metadata.extend(struct.pack('<ddddH', identity * 120., 0., 100., 50., 5))
        metadata.extend(b'Frame')
    for identity in ids:
        metadata.extend(identity.to_bytes(16, 'little'))
    metadata.extend(struct.pack('<I', 0))

    def record(name, ok, detail=None):
        checks.append(dict(name=name, observed=bool(ok), detail=detail))
        (root / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
        if not ok:
            raise AssertionError(name)

    with (root / 'server.log').open('w') as log:
        server = subprocess.Popen([str(args.server.resolve()), '--listen', '127.0.0.1:0',
                                   '--root', str(root / 'authority')],
                                  stdout=log, stderr=subprocess.STDOUT)
        try:
            address = wait(lambda: (m.group(1) if (m := re.search(r'listen=(\S+)',
                            (root / 'server.log').read_text())) else None), 'owned server')
            host, port = address.rsplit(':', 1)
            for index in range(5):
                connection = socket.create_connection((host, int(port)), timeout=5)
                sockets.append(connection)
                if index == 0:
                    message = dict(type='publish', board=board, client=uuid.uuid4().hex,
                                   document=dict(schema=2, counts=[0, 0, 1025], metadata=metadata.hex()),
                                   sources=[])
                else:
                    message = dict(type='hello', board=board, client=uuid.uuid4().hex, revision=0)
                send_message(connection, message)
                assert read_message(connection)['type'] == 'snapshot'
                assert read_message(connection)['type'] == 'lease_snapshot'
            stages = [dict(leases=0, rss_bytes=snapshot(server.pid)['rss_bytes'])]
            for index in range(4):
                objects = [f'{identity:032x}' for identity in ids[index * 256:(index + 1) * 256]]
                send_message(sockets[index], dict(type='lease_acquire', operation=uuid.uuid4().hex,
                                                  base=0, objects=objects))
                for connection in sockets:
                    response = read_message(connection)
                    assert response['type'] == 'lease_changed' and len(response['objects']) == 256
                stages.append(dict(leases=(index + 1) * 256, rss_bytes=snapshot(server.pid)['rss_bytes']))
            send_message(sockets[4], dict(type='lease_acquire', operation=uuid.uuid4().hex,
                                         base=0, objects=[f'{1025:032x}']))
            denied = read_message(sockets[4])
            record('1024 board leases admitted; 1025th refused', denied['type'] == 'lease_denied', denied)
            record('lease RSS growth measured at 0/256/512/768/1024', True, stages)
            before = snapshot(server.pid)
            started = time.monotonic()
            time.sleep(5.2)
            idle = delta(before, snapshot(server.pid), time.monotonic() - started)
            record('server has no lease maintenance timer even across expiry',
                   idle['ticks'] == 0 and all(v == 0 for v in idle['io_delta'].values()), idle)
            # Incoming explicit rejoin lazily expires the table; no heartbeat.
            send_message(sockets[4], dict(type='hello', board=board, client=uuid.uuid4().hex, revision=0))
            response = read_message(sockets[4])
            if response['type'] == 'lease_changed':
                response = read_message(sockets[4])
            assert response['type'] == 'snapshot'
            leases = read_message(sockets[4])
            record('expired full table is empty on explicit rejoin', leases['type'] == 'lease_snapshot'
                   and not leases['leases'], leases)
            (root / 'receipt.json').write_text(json.dumps(dict(
                server_sha256=digest(args.server), harness_sha256=digest(Path(__file__)),
                scope='RSS includes allocator, JSON and outgoing queues; not exact lease entry allocation',
                checks=checks), indent=2) + '\n')
        finally:
            for connection in sockets:
                connection.close()
            server.terminate()
            server.wait(timeout=5)
    print(root / 'receipt.json')


if __name__ == '__main__':
    main()
