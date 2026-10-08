#!/usr/bin/env python3
"""Owned native long input churn plus LOD inventory/idle/save-reopen receipts.

CPU/GPU cache stress is separately asserted by the Rust long-churn tests. This
harness sends thousands of real native input transitions; coalesced camera
positions are not falsely reported as thousands of completed codec transitions.
The first visible image must be the small pixel-art target. Input board must be
a generated linked fixture <=8MiB; only an adjacent owned working copy is saved.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import uuid

from run_idle import observe
from run_image_interaction import digest
from run_local_production import Session
from run_native_image_checks import command

TIERS = ['Thumbnail', 'Medium', 'Detail']
FILTERS = ['Default', 'Smooth', 'Nearest']


def check_report(report, stages, baseline=False):
    frames = report['frames']
    traces = [f['lod_trace'] for f in frames if f.get('lod_trace')]
    if not traces:
        raise AssertionError('missing opt-in LOD trace')
    failures = []
    signatures = {}
    for trace in traces:
        valid = [tier for tier in trace['tiers'] if tier['gpu_resident']]
        finest = max(valid, key=lambda tier: TIERS.index(tier['tier'])) if valid else None
        if finest and trace['displayed'] != finest['tier']:
            failures.append({'source': trace['source'], 'revision': trace['revision'],
                             'projected_edge': trace['projected_edge'],
                             'displayed': trace['displayed'], 'finest': finest['tier']})
        signature = (trace['source'], trace['revision'],
                     round(trace['projected_edge'], 5), trace.get('filtering'),
                     tuple(tier['tier'] for tier in valid))
        previous = signatures.setdefault(signature, trace['displayed'])
        if previous != trace['displayed']:
            failures.append({'history_dependent_signature': repr(signature)})
    receipts = []
    for stage in stages:
        previous = [frame for frame in frames if frame['elapsed_ms'] <= stage['elapsed_ms']]
        if not previous:
            raise AssertionError('no frame for stage '+stage['name'])
        frame = previous[-1]
        trace = frame.get('lod_trace')
        receipt = dict(stage, visible=frame['visible'], quality_resolved=frame['quality_resolved'],
                       pending=frame['supply']['pending'], trace=trace)
        receipts.append(receipt)
        if not baseline and frame['quality_resolved'] != frame['visible']:
            raise AssertionError('settled working set failed to converge: '+json.dumps(receipt))
        if not baseline and stage.get('filtering') and trace:
            if trace.get('filtering') != stage['filtering']:
                raise AssertionError('native filtering action not observed: '+json.dumps(receipt))
    if failures and not baseline:
        raise AssertionError('resident-quality valley: '+json.dumps(failures[:8]))
    return receipts, failures, len(traces), len(signatures)


def idle_receipt(session, seconds):
    before = observe(session.process.pid)
    begin = time.monotonic()
    time.sleep(seconds)
    after = observe(session.process.pid)
    end = time.monotonic()
    return {'begin_ms': (begin-session.started)*1000,
            'end_ms': (end-session.started)*1000, 'seconds': end-begin,
            'ticks': sum(after['tasks'].get(tid, old)['ticks']-old['ticks']
                         for tid, old in before['tasks'].items()),
            'io': {field: after['io'][field]-before['io'][field]
                   for field in ('read_bytes', 'write_bytes')},
            'rss_bytes': after['rss_bytes'], 'threads': len(after['tasks']),
            'before': before, 'after': after}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--board', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--pairs', type=int, default=1200)
    parser.add_argument('--settle-seconds', type=float, default=2)
    parser.add_argument('--initial-filter', choices=FILTERS, default='Default')
    parser.add_argument('--boundary-out', type=int, default=6,
                        help='wheel-out offset before repeated boundary reversal; default300pxfixture→122px')
    parser.add_argument('--potato', action='store_true')
    parser.add_argument('--baseline', action='store_true')
    args = parser.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        parser.error('explicitly owned isolated X11 DISPLAY required')
    if not 1000 <= args.pairs <= 2000 or not 1 <= args.settle_seconds <= 20 or not 0 <= args.boundary_out <= 20:
        parser.error('bounded pairs1000..2000 and settle1..20seconds required')
    binary, board = args.binary.resolve(), args.board.resolve()
    if board.stat().st_size > 8*1024*1024:
        parser.error('use a small generated linked fixture, no embedded corpus copy')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    initial_hash = digest(board)
    # Adjacent copy preserves exact relative LinkedPath semantics. Never edit the
    # caller's board; originals stay at their existing paths and are not copied.
    working = board.parent/f'.phase2a1-lod-{uuid.uuid4().hex}.tack'
    shutil.copyfile(board, working)
    env = dict(os.environ, TACK_NATIVE_NO_WM='1', TACK_TEST_WINDOW_SIZE='800x600',
               WINIT_X11_SCALE_FACTOR='1', TACK_PROFILE_DIR=str(root/'profile'))
    flags = ['--lod-debug'] + (['--potato'] if args.potato else [])
    sessions = []
    stages = []
    idle = []
    current_filter = args.initial_filter
    transitions = {'wheel': 0, 'pan': 0, 'filtering': 0}

    def launch(name):
        session = Session(binary, root, name, ['open', working, *flags], env)
        command('xdotool', 'windowraise', session.window)
        session.focus()
        command('xdotool', 'mousemove', '--window', session.window, 400, 300)
        sessions.append(session)
        return session

    def settle(name, filtering=None):
        time.sleep(args.settle_seconds)
        stages.append({'session': s.name, 'name': name, 'elapsed_ms':
                       (time.monotonic()-s.started)*1000, 'filtering': filtering})

    def wheel(button, count):
        command('xdotool', 'click', '--repeat', count, '--delay', 1, button)
        transitions['wheel'] += count

    def filter_to(name):
        nonlocal current_filter
        s.key('ctrl+a')
        for _ in range((FILTERS.index(name)-FILTERS.index(current_filter)) % 3):
            s.key('alt+t')
            transitions['filtering'] += 1
        current_filter = name
        s.key('Escape')
        settle('filter-'+name, name)

    def churn(pairs):
        # One owned xdotool batch per128 reversals bounds subprocess overhead.
        # 1ms separation allows native event processing without a frame timer.
        for offset in range(0, pairs, 128):
            batch = ['xdotool']
            for _ in range(min(128, pairs-offset)):
                batch += ['click', '--delay', '1', '4', 'click', '--delay', '1', '5']
            subprocess.run(batch, check=True, timeout=20)
            transitions['wheel'] += 2*min(128, pairs-offset)
            if s.process.poll() is not None:
                raise AssertionError('native exited during churn')

    def pan(dx):
        command('xdotool', 'mousemove', '--window', s.window, 400, 300,
                'mousedown', 2, 'mousemove', '--window', s.window, 400+dx, 300,
                'mouseup', 2)
        transitions['pan'] += 1

    try:
        s = launch('churn')
        settle('open')
        filter_to('Nearest')
        wheel(4, 8)
        settle('nearest-detail')
        wheel(5, 8)
        settle('nearest-return-initial')
        if args.boundary_out:
            wheel(5, args.boundary_out)
        settle('nearest-boundary-out')
        churn(args.pairs//2)
        settle('nearest-reversal-churn')
        if args.boundary_out:
            wheel(4, args.boundary_out)
        settle('nearest-boundary-return')
        for _ in range(16):
            pan(300)
            wheel(5, 2)
            wheel(4, 2)
            pan(-300)
        settle('nearest-pan-return')
        filter_to('Smooth')
        if args.boundary_out:
            wheel(5, args.boundary_out)
        settle('smooth-boundary-out')
        churn(args.pairs-args.pairs//2)
        settle('smooth-reversal-churn')
        if args.boundary_out:
            wheel(4, args.boundary_out)
        wheel(4, 8)
        settle('smooth-detail')
        wheel(5, 8)
        settle('smooth-return-initial')
        filter_to('Nearest')
        s.save(working)
        settle('saved-nearest')
        s.shot('settled')
        idle.append(idle_receipt(s, 2))
        s.close()
        saved_hash = digest(working)
        s = launch('reopen')
        settle('reopen-nearest', 'Nearest')
        idle.append(idle_receipt(s, 2))
        s.close()
        reports = []
        for session, observation in zip(sessions, idle):
            report = json.loads(session.report.read_text())
            selected_stages = [stage for stage in stages if stage['session'] == session.name]
            receipts, valleys, traces, signatures = check_report(report, selected_stages, args.baseline)
            quiet = [frame for frame in report['frames'] if
                     observation['begin_ms']+50 <= frame['elapsed_ms'] <= observation['end_ms']-50]
            observation['idle_frames'] = len(quiet)
            if any(observation['io'].values()) or quiet:
                raise AssertionError('idle failed to settle: '+json.dumps(observation))
            reports.append({'name': session.name, 'stages': receipts,
                            'valley_counterexamples': valleys[:16], 'valley_count': len(valleys),
                            'trace_frames': traces, 'distinct_history_signatures': signatures,
                            'first_recognizable_ms': report.get('first_recognizable_ms'),
                            'cpu_callback_ms': [frame['callback_ms'] for frame in report['frames']],
                            'cpu_cache_peak': max(frame['supply']['cpu_bytes'] for frame in report['frames']),
                            'gpu_cache_peak': max(frame['gpu_bytes'] for frame in report['frames']),
                            'peak_pending': max(frame['supply']['pending'] for frame in report['frames']),
                            'uploads': sum(frame['uploads'] for frame in report['frames']),
                            'completed_codec_requests': report['frames'][-1]['supply']['decode_count'],
                            'evictions': report['frames'][-1]['supply']['evictions'], 'idle': observation})
        if digest(working) != saved_hash or digest(board) != initial_hash:
            raise AssertionError('saved reopen/caller original authority changed')
        result = {'binary_sha256': digest(binary), 'board_sha256': initial_hash,
                  'harness_sha256': digest(Path(__file__)), 'working_board': str(working),
                  'saved_board_sha256': saved_hash, 'transitions': transitions, 'reports': reports,
                  'scope': 'real native event/camera churn; actual cache transitions counted separately, '
                           'external settle and idle watchdog only; codec/GPU churn proofs are Rust tests'}
        (root/'summary.json').write_text(json.dumps(result, indent=2)+'\n')
        print(json.dumps({'transitions': transitions, 'reports': len(reports), 'output': str(root)}))
    finally:
        for session in sessions:
            session.kill()


if __name__ == '__main__':
    main()
