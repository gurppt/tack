#!/usr/bin/env python3
"""Owned X11 windows: per-instance idle costs and optional NVIDIA graphics memory."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import xml.etree.ElementTree as ET
from run_local_production import Session
from run_idle import observe
from run_image_interaction import digest


def graphics_memory(pids):
    if not shutil.which('nvidia-smi'):
        return {'available': False, 'reason': 'nvidia-smi unavailable'}
    result = subprocess.run(['nvidia-smi', '-q', '-x'], capture_output=True, timeout=10)
    if result.returncode or len(result.stdout) > 1024 * 1024:
        return {'available': False, 'reason': 'query failed or exceeded 1 MiB'}
    rows = []
    for process in ET.fromstring(result.stdout).findall('.//process_info'):
        if process.findtext('pid') in pids:
            rows.append({'pid': process.findtext('pid'), 'type': process.findtext('type'),
                         'used_gpu_memory': process.findtext('used_memory')})
    return {'available': bool(rows), 'rows': rows,
            'scope': 'NVIDIA graphics-process snapshot after idle; includes driver allocations, not only renderer payloads'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--board', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('TACK_NATIVE_NO_WM') != '1' or os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        parser.error('isolated X11 required')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary, board = args.binary.resolve(), args.board.resolve()
    board_hash = digest(board)
    env = dict(os.environ, TACK_PROFILE_DIR=str(root / 'profile'))
    sessions = []
    try:
        sessions.append(Session(binary, root, 'mixed', ['open', board], env))
        sessions.append(Session(binary, root, 'empty', ['new', root / 'empty.tack'], env))
        time.sleep(3)
        initial = [observe(s.process.pid) for s in sessions]
        time.sleep(5)
        final = [observe(s.process.pid) for s in sessions]
        memory = graphics_memory({str(s.process.pid) for s in sessions})
        rows = []
        for s, a, b in zip(sessions, initial, final):
            ticks = sum(t['ticks'] - a['tasks'].get(tid, t)['ticks'] for tid, t in b['tasks'].items())
            io = {k: b['io'][k] - a['io'][k] for k in a['io']}
            s.close()
            report = json.loads(s.report.read_text())
            rows.append({'name': s.name, 'pid': s.process.pid, 'rss_bytes': b['rss_bytes'], 'threads': len(b['tasks']),
                         'cpu_ticks_5s': ticks, 'cpu_percent_one_core': ticks / os.sysconf('SC_CLK_TCK') / 5 * 100,
                         'io_delta': io, 'renderer_payload_bytes': report['frames'][-1]['gpu_bytes'],
                         'annotation_buffers_bytes': report['annotation_resources'],
                         'report_sha256': digest(s.report)})
        result = {'binary_sha256': digest(binary), 'harness_sha256': digest(__file__), 'board_sha256': board_hash,
                  'rows': rows, 'graphics_memory': memory, 'scope': 'two independent settled native processes; 5s observation; no total-machine memory subtraction'}
        (root / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
        if digest(board) != board_hash or any(r['cpu_ticks_5s'] > 5 or any(r['io_delta'].values()) for r in rows):
            raise AssertionError('independent idle or authority gate')
    finally:
        for s in sessions:
            s.kill()


if __name__ == '__main__':
    main()
