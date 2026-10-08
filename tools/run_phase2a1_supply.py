#!/usr/bin/env python3
"""Serial huge JPEG corrective measurements on an owned display; include helper descendants."""
import argparse
import json
import os
from pathlib import Path
import time
from run_local_production import Session
from run_native_image_checks import command
from run_idle import observe
from run_image_interaction import digest


def ticks(value):
    return sum(t['ticks'] for t in value['tasks'].values())


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True)
    p.add_argument('--board',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--potato',action='store_true')
    p.add_argument('--tiles',action='store_true')
    p.add_argument('--dense',action='store_true')
    p.add_argument('--zoom',action='store_true')
    p.add_argument('--zoom-count',type=int,default=44)
    p.add_argument('--lod-debug',action='store_true')
    p.add_argument('--settle-budget',type=float,default=25.)
    a=p.parse_args()
    if os.environ.get('DISPLAY') in (None,':0',':0.0'):p.error('isolated owned display required')
    root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
    flags=[]
    if a.potato:flags+=['--potato']
    if a.tiles:flags+=['--huge-tiles']
    if a.dense:flags+=['--dense-view']
    if a.lod_debug:flags+=['--lod-debug']
    env=dict(os.environ,TACK_PROFILE_DIR=str(root/'profile'),TACK_TEST_WINDOW_SIZE='800x600',TACK_NATIVE_NO_WM='1')
    temporary=root/'temporary';temporary.mkdir()
    env['TMPDIR']=str(temporary)
    s=Session(a.binary.resolve(),root,'supply',['open',a.board.resolve(),*flags],env)
    samples=[];stages=[]
    def pan(direction):
        for _ in range(3):
            command('xdotool','mousemove','--window',s.window,'400','300','mousedown','2','mousemove','--window',s.window,str(400+direction*300),'300','mouseup','2')
        command('xdotool','mousemove','--window',s.window,'400','300')
    def sample():
        value=observe(s.process.pid)
        status=Path(f'/proc/{s.process.pid}/status').read_text().splitlines()
        hwm=next(int(line.split()[1])*1024 for line in status if line.startswith('VmHWM:'))
        item={'elapsed_ms':(time.monotonic()-s.started)*1000,'rss_bytes':value['rss_bytes'],'hwm_bytes':hwm,'ticks':ticks(value),'io':value['io']}
        children=[]
        for task in Path(f'/proc/{s.process.pid}/task').iterdir():
            try: ids=(task/'children').read_text().split()
            except FileNotFoundError: continue
            for pid in ids:
                try:
                    status=Path(f'/proc/{pid}/status').read_text().splitlines()
                    values={line.split(':',1)[0]:line.split(':',1)[1].strip() for line in status}
                    if 'jpeg' not in values.get('Name','') and 'djpeg' not in values.get('Name',''):continue
                    children.append({'pid':int(pid),'rss_bytes':int(values.get('VmRSS','0 kB').split()[0])*1024,'hwm_bytes':int(values.get('VmHWM','0 kB').split()[0])*1024})
                except (FileNotFoundError,ProcessLookupError):pass
        item['decoder_children']=children
        samples.append(item);return item
    def settle(name):
        begin=time.monotonic();previous=sample();quiet=None;converged=False
        while time.monotonic()-begin<a.settle_budget:
            time.sleep(.05);current=sample()
            # External bounded quiet detector is not proof of quality; final frame reports are separate.
            calm=current['io']['rchar']==previous['io']['rchar'] and current['io']['wchar']==previous['io']['wchar'] and current['ticks']-previous['ticks']<=1
            if calm:
                if quiet is None:quiet=time.monotonic()
                elif time.monotonic()-quiet>=1.5:converged=True;break
            else:quiet=None
            previous=current
        cache_bytes=sum(path.stat().st_size for path in temporary.rglob('*.png') if path.is_file())
        stages.append({'derived_png_bytes_at_settle':cache_bytes,'name':name,'quiet_detected':converged,'wait_seconds':time.monotonic()-begin,'elapsed_ms':samples[-1]['elapsed_ms']})
    try:
        command('xdotool','mousemove','--window',s.window,'400','300')
        settle('cold-open');s.shot('cold')
        if a.zoom:
            command('xdotool','click','--repeat',str(a.zoom_count),'--delay','15','4');settle('deep-zoom');s.shot('detail')
            pan(-1);settle('distant-pan')
            pan(1);settle('back-warm')
            command('xdotool','click','--repeat',str(a.zoom_count),'--delay','15','5');settle('overview');s.shot('overview')
        idle_first=sample();time.sleep(2.);idle_last=sample()
        idle_start,idle_end=idle_first['elapsed_ms'],idle_last['elapsed_ms']
        s.close()
    finally:
        if s.process.poll() is None:s.kill()
    data=json.loads(s.report.read_text());frames=data['frames']
    final=frames[-1] if frames else {}
    quiet_frames=[f for f in frames if idle_start-100<=f['elapsed_ms']<=idle_end+100]
    def percentile(values,q):
        if not values:return None
        values=sorted(values);return values[min(len(values)-1,int((len(values)-1)*q))]
    if a.zoom and final.get('zoom',0)>2:
        raise AssertionError('zoom-out did not return to overview: '+str(final.get('zoom')))
    summary={'binary_sha256':digest(a.binary),'board_sha256':digest(a.board),'potato':a.potato,'tiles_opt_in':a.tiles,'dense':a.dense,
        'stages':stages,'decoder_sampled_rss_peak':max((c['rss_bytes'] for x in samples for c in x['decoder_children']),default=0),'decoder_sampled_hwm_peak':max((c['hwm_bytes'] for x in samples for c in x['decoder_children']),default=0),'decoder_sampling_note':'50ms proc samples can miss short-lived decoder peaks; analytic native row/scratch bounds and tests are separate','peak_rss_bytes':max(x['rss_bytes'] for x in samples),'peak_hwm_bytes':max(x['hwm_bytes'] for x in samples),
        'first_frame_ms':data.get('first_frame_ms'),'first_recognizable_ms':data.get('first_recognizable_ms'),'useful_ms':data.get('ordinary_view_80_percent_ms'),
        'errors':data.get('errors'),'adapter':data.get('adapter'),'final':final,'total_frames':len(frames),
        'cpu_payload_peak':data.get('cpu_payload_peak'),'source_bytes':data.get('source_bytes_before_detail'),'codec_requests':data.get('codec_requests'),
        'decoded_bytes':data.get('decoded_bytes'),'first_tiled_detail_ms':next((f['elapsed_ms'] for f in frames if f.get('tiles_requested',0)>0 and f.get('tiles_ready',0)==f.get('tiles_requested',0)),None),'peak_gpu_bytes':max((f['gpu_bytes'] for f in frames),default=0),
        'callback_ms_p50':percentile([f['callback_ms'] for f in frames],.5),'callback_ms_p95':percentile([f['callback_ms'] for f in frames],.95),'callback_ms_p99':percentile([f['callback_ms'] for f in frames],.99),
        'callback_ms_max':max((f['callback_ms'] for f in frames),default=0),'gpu_pass_ms_p95':percentile([f['pass_ms'] for f in data.get('gpu_samples',[])],.95),
        'idle_ticks':idle_last['ticks']-idle_first['ticks'],'idle_io':{k:idle_last['io'][k]-idle_first['io'][k] for k in idle_first['io']},'idle_frames':len(quiet_frames),
        'derived_png_bytes_max_settled':max((x['derived_png_bytes_at_settle'] for x in stages),default=0),
        'scope':'Observed RTX2060/Linux native800x600; quiet is external detector, inspect quality/tilecounts independently; no low-end feasibility claim'}
    (root/'samples.json').write_text(json.dumps(samples,indent=2)+'\n');(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps({k:v for k,v in summary.items() if k not in ('final','stages')},indent=2))

if __name__=='__main__':main()
