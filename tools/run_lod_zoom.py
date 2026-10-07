#!/usr/bin/env python3
"""Serial ordinary-window zoom oscillation; watchdog lives only in this harness."""
import argparse
import json
import os
from pathlib import Path
import time
from run_local_production import Session
from run_native_image_checks import command
from run_idle import observe
from run_image_interaction import digest


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--board', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--window-size', default='800x600')
    p.add_argument('--baseline', action='store_true')
    p.add_argument('--no-diagnostics', action='store_true')
    a = p.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        p.error('owned isolated display required')
    root = a.output.resolve(); root.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ, TACK_PROFILE_DIR=str(root/'profile'), TACK_TEST_WINDOW_SIZE=a.window_size, TACK_NATIVE_NO_WM='1')
    flags = [] if a.baseline or a.no_diagnostics else ['--lod-debug']
    s = Session(a.binary.resolve(), root, 'zoom', ['open', a.board.resolve(), *flags], env)
    start = s.started; stages = []
    def settle(name, seconds=1.):
        time.sleep(seconds); stages.append({'name':name,'elapsed_ms':(time.monotonic()-start)*1000})
    def wheel(button, count, delay=.015):
        command('xdotool','click','--repeat',str(count),'--delay',str(int(delay*1000)),str(button))
    try:
        w,h = map(int,a.window_size.split('x'))
        command('xdotool','mousemove','--window',s.window,w//2,h//2)
        settle('open',2.)
        wheel(4,8); settle('strong-in'); s.shot('hd')
        wheel(5,10); settle('partial-out'); s.shot('medium')
        wheel(5,12); settle('further-out'); s.shot('overview')
        wheel(4,14); settle('back-in')
        for _ in range(15): wheel(5,2,.001); wheel(4,2,.001)
        settle('rapid-alternation')
        for _ in range(12): wheel(5,1); time.sleep(.075)
        settle('progressive-out')
        for _ in range(12): wheel(4,1); time.sleep(.075)
        settle('progressive-in')
        command('xdotool','mousedown','2');command('xdotool','mousemove_relative','--',40,20);command('xdotool','mouseup','2')
        wheel(5,6);wheel(4,6);settle('pan-and-settle',3.)
        initial=observe(s.process.pid); begin=time.monotonic();time.sleep(2.); final=observe(s.process.pid);end=time.monotonic()
        s.close()
    finally:
        if s.process.poll() is None:s.kill()
    data=json.loads(s.report.read_text());frames=data['frames']
    # Conservatively include launch/clock uncertainty; inspect the entire idle interval.
    quiet=[f for f in frames if (begin-start)*1000-100 <= f['elapsed_ms'] <= (end-start)*1000+100]
    last=frames[-1]
    if not a.baseline and last['quality_resolved'] != last['visible']:
        raise AssertionError('test-only watchdog: final visible working set did not converge')
    trace=[f['lod_trace'] for f in frames if f.get('lod_trace')]
    if not a.baseline:
        if not a.no_diagnostics and not trace:raise AssertionError('no opt-in LOD traces')
        for stage in stages:
            settled=[f for f in frames if f['elapsed_ms']<=stage['elapsed_ms']]
            if settled and settled[-1]['quality_resolved']!=settled[-1]['visible']:raise AssertionError('stage convergence: '+stage['name'])
        for t in trace:
            adequate=[r for r in t['tiers'] if r['gpu_resident'] and ['Thumbnail','Medium','Detail'].index(r['tier'])>=['Thumbnail','Medium','Detail'].index(t['desired'])]
            if adequate and (t['displayed'] is None or ['Thumbnail','Medium','Detail'].index(t['displayed'])<['Thumbnail','Medium','Detail'].index(t['desired'])):
                raise AssertionError('suitable resident was ignored')
    ticks=sum(final['tasks'].get(t,old)['ticks']-old['ticks'] for t,old in initial['tasks'].items())
    io={k:final['io'][k]-initial['io'][k] for k in initial['io']}
    (root/'idle-raw.json').write_text(json.dumps({'initial':initial,'final':final,'ticks':ticks,'io':io,'quiet':quiet},indent=2))
    if io['read_bytes'] or io['write_bytes'] or quiet:raise AssertionError('settled idle work')
    receipt={'binary_sha256':digest(a.binary),'board_sha256':digest(a.board),'stages':stages,
             'final_quality':last['quality_resolved'],'visible':last['visible'],'idle_ticks':ticks,'idle_io':io,
             'idle_seconds':end-begin,'trace_frames':len(trace),'decode_count':last['supply']['decode_count'],
             'peak_cpu_bytes':max(f['supply']['cpu_bytes'] for f in frames),'peak_gpu_bytes':max(f['gpu_bytes'] for f in frames),
             'uploads':sum(f['uploads'] for f in frames),'upload_bytes':sum(f['upload_bytes'] for f in frames),
             'codec_requests':data.get('codec_requests'),'decoded_bytes':data.get('decoded_bytes'),
             'max_pending':max(f['supply']['pending'] for f in frames),'total_frames':len(frames),
             'scope':'ordinary untimed native input, bounded external settle/idle watchdog; 8 mixed/grouped/rotated generated sources'}
    (root/'summary.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt,indent=2))

if __name__=='__main__':main()
