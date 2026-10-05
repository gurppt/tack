#!/usr/bin/env python3
"""1G serial native supply runs; raw frame/event/GPU distributions retained."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time
from run_image_interaction import digest


def percentile(values, p):
    values = sorted(values)
    return values[int((len(values)-1)*p)] if values else None


def summarize(data):
    frames = data['frames']
    stats = {key: {'p50': percentile([f[key] for f in frames], .5),
                   'p99': percentile([f[key] for f in frames], .99),
                   'max': max((f[key] for f in frames), default=None)}
             for key in ('query_ms', 'scene_ms', 'supply_ms', 'encode_ms', 'submit_ms',
                         'poll_ms', 'acquire_ms', 'present_ms', 'callback_ms')}
    episodes = []
    for frame in frames:
        # The scripted test alternates first/last objects every two seconds.
        epoch = int(max(0, frame['elapsed_ms']/1000-1)/2)
        if not episodes or episodes[-1]['epoch'] != epoch:
            episodes.append({'epoch': epoch, 'entered_ms': frame['elapsed_ms'],
                             'useful_delay_ms': None, 'quality_delay_ms': None})
        e = episodes[-1]
        if frame['visible'] and frame['recognizable']*100 >= frame['visible']*80 and e['useful_delay_ms'] is None:
            e['useful_delay_ms'] = frame['elapsed_ms']-e['entered_ms']
        if frame['visible'] and frame['quality_resolved'] == frame['visible'] and e['quality_delay_ms'] is None:
            e['quality_delay_ms'] = frame['elapsed_ms']-e['entered_ms']
    return {'distribution_ms': stats, 'episodes': episodes,
            'first_useful_ms': data['ordinary_view_80_percent_ms'],
            'max_visible': max((f['visible'] for f in frames), default=0),
            'max_detailed': max((f['detailed'] for f in frames), default=0),
            'peak_gpu_payload': max((f['gpu_bytes'] for f in frames), default=0),
            'peak_cpu_payload': data['cpu_payload_peak'],
            'peak_pending': data['peak_pending'], 'peak_queued': data['peak_queued'],
            'cpu_evictions': data['cpu_evictions'], 'discarded': data['discarded'],
            'reprioritized': data['reprioritized'], 'source_bytes': data['source_bytes_before_detail'],
            'container_bytes': data['container_bytes'], 'metadata_ms': data['metadata_load_ms'],
            'gpu_ms_p99': percentile([s['pass_ms'] for s in data['gpu_samples']], .99)}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--fixtures', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        p.error('isolated X11 required')
    root = a.output.resolve(); root.mkdir(parents=True, exist_ok=False)
    binary = a.binary.resolve(); fixtures = a.fixtures.resolve()
    receipt = {'binary_sha256': digest(binary), 'harness_sha256': digest(__file__), 'runs': [],
               'scope': 'generated shared-source fixtures, warm OS caches, isolated X11; epochs censored if unfinished; no human/monitor latency claim'}
    for name, fixture, flags in [
            ('1k', 'images-1k', []), ('5k', 'images-5k', []),
            ('50k', 'images-50k', []), ('10k-shapes', 'shapes-10k', []),
            ('mixed', 'mixed', []), ('sparse', 'sparse', []),
            ('potato-5k', 'images-5k', ['--potato']),
            ('sparse-immediate', 'sparse', ['--present-immediate']),
            ('dense-5k', 'images-5k', ['--dense-view']),
            ('dense-potato', 'images-5k', ['--dense-view', '--potato']),
            ('oversubscribed-highlod', 'dense-highlod', []),
            ('oversubscribed-potato', 'dense-highlod', ['--potato'])]:
        report = root/f'{name}.json'; board = fixtures/f'{fixture}.tack'
        command = [str(binary), 'open', str(board), '--seconds', '17',
                   '--output', str(report), *flags]
        if not name.startswith(('dense-', 'oversubscribed-')):
            command.append('--supply-stress')
        env = dict(os.environ, TACK_PROFILE_DIR=str(root/f'profile-{name}'))
        peak = 0; samples = []
        with (root/f'{name}.log').open('w') as log:
            process = subprocess.Popen(command, env=env, stdout=log, stderr=subprocess.STDOUT)
            start = time.monotonic()
            try:
                while process.poll() is None:
                    if time.monotonic()-start > 45:
                        raise RuntimeError(f'{name} timeout')
                    try:
                        rows = Path(f'/proc/{process.pid}/status').read_text().splitlines()
                        rss = next(int(s.split()[1])*1024 for s in rows if s.startswith('VmRSS:'))
                        peak = max(peak, rss); samples.append({'elapsed_ms': (time.monotonic()-start)*1000, 'rss_bytes': rss})
                    except (FileNotFoundError, ProcessLookupError, StopIteration):
                        pass
                    time.sleep(.02)
                if process.returncode:
                    raise RuntimeError(f'{name} exit {process.returncode}')
            finally:
                if process.poll() is None:
                    process.kill(); process.wait(timeout=4)
        data = json.loads(report.read_text())
        summary = summarize(data)
        (root/f'{name}-rss.json').write_text(json.dumps(samples))
        receipt['runs'].append({'name': name, 'board_sha256': digest(board), 'report_sha256': digest(report),
                                'peak_rss_bytes': peak, **summary})
        (root/'summary.json').write_text(json.dumps(receipt, indent=2)+'\n')
        print(name, summary['max_detailed'], summary['peak_pending'], flush=True)
        if summary['peak_pending'] > (4 if '--potato' in flags else 16):
            raise AssertionError('request cap')
        if summary['peak_gpu_payload'] > (16 if '--potato' in flags else 128)*1024*1024:
            raise AssertionError('GPU cap')
        if fixture != 'shapes-10k' and not name.startswith('dense-') and not summary['max_detailed']:
            raise AssertionError('no high-LOD observed')


if __name__ == '__main__':
    main()
