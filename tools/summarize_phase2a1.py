#!/usr/bin/env python3
"""Compact reproducible Phase2A1 receipts; never run codecs or copy test data."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
MATRIX = [f'axis-{axis}-v2' for axis in (4096,6000,8192,12000,16000,24000,32000,50000)] + [
    '50k-potato-v2','50k-reopen-v2','jpeg-ordinary-v2','jpeg-png-v2','multiple-jpeg-v2']
MEASURED_SHA = 'c9f7e4eae976ab6f49b8cc012cf1d7273848385187b61b62d2553aeca30a43f5'


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda: source.read(65536), b''):
            h.update(chunk)
    return h.hexdigest()


def load(path):
    return json.loads(path.read_text())


def distribution(values):
    values = sorted(value for value in values if value is not None)
    if not values:
        return None
    return {**{f'p{p}': values[min(len(values)-1, int((len(values)-1)*p/100))]
               for p in (50,95,99)}, 'max': values[-1], 'count': len(values)}


def native_pose(frame):
    return (round(frame['zoom'],8), tuple(round(value,6) for value in frame['camera']))


def snapshot(frame):
    return {'native_elapsed_ms': frame['elapsed_ms'], 'camera': frame['camera'], 'zoom': frame['zoom'],
            **{key: frame.get(key) for key in ('visible','recognizable','quality_resolved','tiles_requested','tiles_ready','gpu_bytes')},
            'supply': {key: frame['supply'][key] for key in (
                'pending','queued','cpu_bytes','codec_requests','decode_count','evictions','discarded',
                'source_bytes','container_bytes','decoded_bytes')}}


def camera_evidence(frames):
    # Identify actual stable camera states in the native clock. External Session
    # stage timestamps use another origin and must not select native frame rows.
    groups = []
    for frame in frames:
        if not groups or native_pose(groups[-1][-1]) != native_pose(frame):
            groups.append([frame])
        else:
            groups[-1].append(frame)
    maximum = max(frame['zoom'] for frame in frames)
    deepest = [group for group in groups if group[-1]['zoom'] == maximum]
    evidence = {'cold_camera': snapshot(groups[0][-1]), 'final_camera': snapshot(frames[-1])}
    if deepest:
        evidence['deepest_camera'] = snapshot(deepest[0][-1])
    if len(deepest) >= 3 and native_pose(deepest[0][-1]) == native_pose(deepest[2][-1]):
        far, returned = deepest[1][-1], deepest[2][-1]
        evidence['far_pan'] = snapshot(far)
        evidence['exact_return'] = snapshot(returned)
        evidence['return_new_codec_jobs'] = returned['supply']['codec_requests']-far['supply']['codec_requests']
        evidence['return_source_bytes'] = returned['supply']['source_bytes']-far['supply']['source_bytes']
        evidence['return_container_bytes'] = returned['supply']['container_bytes']-far['supply']['container_bytes']
        if evidence['return_new_codec_jobs'] or evidence['return_source_bytes'] or evidence['return_container_bytes']:
            evidence['reuse_observed'] = False
        else:
            evidence['reuse_observed'] = True
    return evidence


def supply_receipt(folder):
    summary, report = load(folder/'summary.json'), load(folder/'supply.json')
    frames = report['frames']
    final = frames[-1]
    if summary['errors'] or report.get('errors') or report.get('load_failed') or any(
            report.get(key, 0) for key in ('source_missing','source_changed','source_unavailable','source_foreign')):
        raise AssertionError(f'{folder.name}: image error is not successful detail')
    if not final['visible'] or final['quality_resolved'] != final['visible'] or final['supply']['pending']:
        raise AssertionError(f'{folder.name}: unresolved final working set')
    if summary['idle_frames'] or any(summary['idle_io'].values()):
        raise AssertionError(f'{folder.name}: idle not quiet')
    if not all(stage['quiet_detected'] for stage in summary['stages']):
        raise AssertionError(f'{folder.name}: external quiet watchdog expired')
    camera = camera_evidence(frames)
    deepest = camera.get('deepest_camera')
    if summary['first_tiled_detail_ms'] is not None:
        if not deepest or not deepest['tiles_requested'] or deepest['tiles_ready'] != deepest['tiles_requested']:
            raise AssertionError(f'{folder.name}: deepest admitted tile set did not converge')
    cpu_budget, gpu_budget = ((8,16) if summary['potato'] else (64,128))
    if max(frame['supply']['cpu_bytes'] for frame in frames) > cpu_budget*1024*1024:
        raise AssertionError(f'{folder.name}: CPU payload budget')
    if max(frame['gpu_bytes'] for frame in frames) > gpu_budget*1024*1024:
        raise AssertionError(f'{folder.name}: GPU payload budget')
    return {'name': folder.name, **{key: summary.get(key) for key in (
        'binary_sha256','board_sha256','adapter','potato','tiles_opt_in','dense',
        'first_frame_ms','first_recognizable_ms','useful_ms','first_tiled_detail_ms',
        'peak_rss_bytes','peak_hwm_bytes','decoder_sampled_rss_peak','decoder_sampled_hwm_peak',
        'cpu_payload_peak','peak_gpu_bytes','source_bytes','codec_requests','decoded_bytes',
        'derived_png_bytes_max_settled','idle_ticks','idle_frames','idle_io')},
        'cpu_scene_ms': distribution([frame['cpu_ms'] for frame in frames]),
        'callback_including_present_ms': distribution([frame['callback_ms'] for frame in frames]),
        'gpu_pass_ms': distribution([sample['pass_ms'] for sample in report.get('gpu_samples',[])]),
        'event_callback_ms': distribution(report.get('event_samples_ms',[])),
        'wheel_callback_ms': distribution(report.get('wheel_samples_ms',[])),
        'peak_pending': max(frame['supply']['pending'] for frame in frames),
        'peak_queued': max(frame['supply']['queued'] for frame in frames),
        'peak_uploads_per_frame': max(frame['uploads'] for frame in frames),
        'peak_upload_bytes_per_frame': max(frame['upload_bytes'] for frame in frames),
        'uploads': sum(frame['uploads'] for frame in frames),
        'camera_evidence': camera,
        'external_watchdog_stages': summary['stages']}


def build_fields(path):
    return dict(line.split(': ',1) for line in path.read_text().splitlines() if ': ' in line)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=REPO/'benchmark-results/phase2a1')
    parser.add_argument('--output', type=Path, default=REPO/'docs/measurements/phase2a1.json')
    args = parser.parse_args()
    root = args.root.resolve()
    receipts = {}
    def record(path):
        receipts[str(path.relative_to(root))] = digest(path)
    matrix = []
    for name in MATRIX:
        folder = root/name
        row = supply_receipt(folder)
        if row['binary_sha256'] != MEASURED_SHA:
            raise AssertionError(f'{name}: measured matrix binary changed')
        matrix.append(row)
        for filename in ('summary.json','supply.json','samples.json'):
            record(folder/filename)
    result = {'schema':1,'generated_utc':datetime.now(timezone.utc).isoformat(),
              'acceptance_status':'in progress: final native/regression/CI receipts are separate',
              'baseline': load(root/'baseline/receipt.json'),
              'matrix_build': build_fields(root/'measured-build/BUILD.txt'),
              'matrix':matrix,'final_replays':[], 'supplementary':{},
              'scope': {'host':'Linux RTX2060 native800x600 isolated X11; serial tests',
                        'clocks':'Recognizable/tile/frame timings use native CPU submission clock; '
                                 'external stage elapsed uses Session clock and is not joined by timestamp.',
                        'memory':'CPU/GPU numbers are resident payloads; whole process/child RSS is separate. '
                                 '50ms child sampling can miss short-lived peaks; no scratch-allocation trace.',
                        'codec':'Each uncached JPEG tile scans sequential entropy. No random access/persistent pyramid.',
                        'page_cache':'warm or unspecified; no physical cold-I/O, low-end or Windows desktop timing claim'},
              'raw_receipts_sha256':receipts}
    for relative in ('baseline/receipt.json','baseline/gate.log','measured-source.json',
                     'measured-build/BUILD.txt','gate-02.log','subsampled-tests.log'):
        path = root/relative
        if path.exists():
            record(path)
    result['source_manifest'] = {'path':str((root/'measured-source.json').relative_to(REPO)),
                                 'sha256':digest(root/'measured-source.json'),
                                 'files':len(load(root/'measured-source.json'))}
    # New final-code replay folders are explicitly named, never mix rejected
    # pilots or v1 input-overlap runs into accepted performance measurements.
    for folder in sorted(root.glob('final-*')):
        if (folder/'summary.json').exists() and (folder/'supply.json').exists():
            result['final_replays'].append(supply_receipt(folder))
            for filename in ('summary.json','supply.json','samples.json'):
                record(folder/filename)
    for name in ('native-lod','grouped','local-regression','native2a'):
        folder = root/name
        path = folder/'summary.json'
        if path.exists():
            data=load(path)
            record(path)
            if name == 'native-lod':
                result['supplementary'][name] = {
                    key:data.get(key) for key in ('binary_sha256','board_sha256','harness_sha256',
                                                  'transitions','saved_board_sha256','scope')}
                result['supplementary'][name]['reports'] = [
                    {key:report.get(key) for key in ('name','valley_count','trace_frames',
                        'distinct_history_signatures','first_recognizable_ms','cpu_cache_peak',
                        'gpu_cache_peak','peak_pending','uploads','completed_codec_requests','evictions')}
                    | {'callback_ms':distribution(report.get('cpu_callback_ms',[])),
                       'idle':{key:report['idle'].get(key) for key in ('seconds','ticks','io','rss_bytes','threads','idle_frames')}}
                    for report in data['reports']]
            elif name == 'local-regression':
                result['supplementary'][name] = {'scope':data['scope'],'runs':[
                    {key:run.get(key) for key in ('name','mode','binary_sha256','board_sha256',
                        'quiet_detected','native_startup_ms','first_frame_ms','first_recognizable_ms',
                        'useful_ms','gpu_bytes_peak','callbacks','idle_redraws')}
                    | {'idle':{key:run['idle'].get(key) for key in ('seconds','rss_after','threads','ticks',
                        'cpu_percent_one_core','socket_records','io_delta')}} for run in data['runs']]}
            else:
                result['supplementary'][name] = data
        provenance,checks=folder/'provenance.json',folder/'checks.json'
        if provenance.exists() and checks.exists():
            record(provenance);record(checks)
            entries=load(checks)
            if not all(entry['observed'] for entry in entries):
                raise AssertionError(f'{name}: native assertion failed')
            result['supplementary'][name] = {'provenance':load(provenance),
                                           'checks_passed':len(entries),
                                           'check_names':[entry['name'] for entry in entries]}
    # Current human build is a separate identity: do not imply the earlier full
    # matrix was rerun after final arithmetic/pixel corrections.
    if (REPO/'bin/BUILD.txt').exists():
        result['current_build'] = build_fields(REPO/'bin/BUILD.txt')
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'matrix_runs':len(matrix),'final_replays':len(result['final_replays']),
                      'supplementary':list(result['supplementary']),'output':str(args.output)}))


if __name__ == '__main__':
    main()
