#!/usr/bin/env python3
"""Paired empty-board background/grid costs; owned native X11, no fixture duplication."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time
from run_local_production import Session
from run_image_interaction import digest
from run_supply import percentile


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--baseline', type=Path, required=True)
    p.add_argument('--current', type=Path, required=True)
    p.add_argument('--board', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    a = p.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0') or os.environ.get('TACK_NATIVE_NO_WM') != '1':
        p.error('requires explicitly owned isolated X11 display')
    root = a.output.resolve(); root.mkdir(parents=True, exist_ok=False)
    baseline, current, board = a.baseline.resolve(), a.current.resolve(), a.board.resolve()
    before = digest(board)
    rows = []
    receipt = {'scope': 'paired native empty-board user-driven pan; startup frames excluded, warm caches, serialized GPU ownership; GPU timestamp pass cost distinct from CPU submit/present',
               'baseline_sha256':digest(baseline), 'current_sha256':digest(current), 'board_sha256':before, 'runs':rows}
    def persist():
        (root/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    for pair in range(2):
        for size in ('800x600', '1024x768', '1600x900'):
            for grid in (False, True):
                for name,binary in ([('baseline',baseline),('current',current)] if pair == 0 else [('current',current),('baseline',baseline)]):
                    case = f'{pair}-{size}-grid-{int(grid)}-{name}'
                    env = dict(os.environ, TACK_PROFILE_DIR=str(root/'profiles'/case), XDG_DATA_HOME=str(root/'data'), TACK_TEST_WINDOW_SIZE=size)
                    s = Session(binary, root, case, ['open',board],env)
                    try:
                        if grid: s.key('g')
                        subprocess.run(['xdotool','mousemove','--window',s.window,'100','100','mousedown','2'],check=True,timeout=3)
                        for n in range(160):
                            subprocess.run(['xdotool','mousemove','--window',s.window,str(100 + n%40),str(100 + n%20)],check=True,timeout=3)
                            time.sleep(.014)
                        subprocess.run(['xdotool','mouseup','2'],check=True,timeout=3)
                        time.sleep(.15)
                        s.close()
                    finally:
                        s.kill()
                    report = json.loads(s.report.read_text())
                    if report['window_size'] != list(map(int,size.split('x'))) or report['spatial']['grid'] != grid:
                        raise AssertionError('requested viewport/grid not observed')
                    if report['annotations'] or any(f['visible'] for f in report['frames']):
                        raise AssertionError('background measurement must have an empty board')
                    frames = report['frames'][3:]
                    gpu = report['gpu_samples'][3:]
                    if len(frames)<30 or len(gpu)<30: raise AssertionError('insufficient measured frames/GPU timestamps')
                    metrics = {}
                    for key in ('scene_ms','encode_ms','submit_ms','cpu_ms','present_ms','callback_ms'):
                        values = [f[key] for f in frames]
                        metrics[key] = {'p50':percentile(values,.5),'p99':percentile(values,.99),'max':max(values)}
                    values = [f['pass_ms'] for f in gpu]
                    metrics['gpu_pass_ms'] = {'p50':percentile(values,.5),'p99':percentile(values,.99),'max':max(values)}
                    rows.append({'pair':pair,'size':size,'grid':grid,'binary':name,'report':s.report.name,'report_sha256':digest(s.report),
                                 'frames':len(frames),'gpu_samples':len(gpu),'adapter':report['adapter'],'backend':report['backend'],'metrics':metrics})
                    persist(); print(case,metrics['gpu_pass_ms'],flush=True)
    if digest(board)!=before: raise AssertionError('performance changed board authority')
    persist()

if __name__ == '__main__': main()
