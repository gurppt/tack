#!/usr/bin/env python3
import argparse, json, os, subprocess, time
from pathlib import Path
from run_local_production import Session
from run_idle import observe
from run_native_image_checks import command
from run_image_interaction import digest
parser=argparse.ArgumentParser(description='Untimed local supply idle gates on an isolated X11 display')
parser.add_argument('--binary',type=Path,required=True)
parser.add_argument('--fixtures',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
parser.add_argument('--baseline',action='store_true')
cfg=parser.parse_args()
if os.environ.get('DISPLAY') in (None,':0',':0.0'):parser.error('isolated X11 required')
root=cfg.output.resolve();root.mkdir(parents=True,exist_ok=False)
binary=cfg.binary.resolve();fixtures=cfg.fixtures.resolve()
env=dict(os.environ,TACK_PROFILE_DIR=str(root/'profile')); rows=[]
for name,args in [('empty',['new',root/'empty.tack']),('small',['open',fixtures/'small.tack']),('1k',['open',fixtures/'images-1k.tack']),('5k',['open',fixtures/'images-5k.tack']),('dense-potato',['open',fixtures/'images-5k.tack','--dense-view','--potato']),('overloaded-potato',['open',fixtures/'dense-highlod.tack','--potato'])]:
 if cfg.baseline and 'potato' in name:continue
 launched=time.monotonic()
 s=Session(binary,root,name,args,env)
 spy=None
 try:
  settle_start=time.monotonic()
  while True:
   previous=observe(s.process.pid);time.sleep(1);current=observe(s.process.pid)
   tid=str(s.process.pid)
   quiet=current['tasks'][tid]['voluntary_ctxt_switches']==previous['tasks'][tid]['voluntary_ctxt_switches'] and all(current['io'][k]==previous['io'][k] for k in previous['io'])
   if quiet:break
   if time.monotonic()-settle_start>30:raise AssertionError('supply failed to settle within 30s')
  settled=time.monotonic()-launched
  lf=(root/f'{name}-properties.log').open('w');spy=subprocess.Popen(['xprop','-spy','-id',s.window,'WM_NAME','_NET_WM_NAME'],stdout=lf,stderr=subprocess.STDOUT);time.sleep(.2)
  lines=len((root/f'{name}-properties.log').read_text().splitlines())
  observation_start=(time.monotonic()-launched)*1000
  a=observe(s.process.pid);time.sleep(5);b=observe(s.process.pid)
  observation_end=(time.monotonic()-launched)*1000
  properties=len((root/f'{name}-properties.log').read_text().splitlines())-lines
  spy.terminate();spy.wait(timeout=3);spy=None;lf.close()
  s.close();d=json.loads(s.report.read_text())
  span=[f for f in d['frames'] if observation_start+200<=f['elapsed_ms']<=observation_end-200]
  row={'name':name,'settle_seconds':settled,'tasks_delta':[{ 'name':t['name'],'ticks':t['ticks']-a['tasks'].get(k,t)['ticks'],'voluntary_switches':t['voluntary_ctxt_switches']-a['tasks'].get(k,t)['voluntary_ctxt_switches']} for k,t in b['tasks'].items()],'rss_bytes':b['rss_bytes'],'threads':len(b['tasks']),'cpu_ticks_5s':sum(v['ticks']-a['tasks'].get(k,v)['ticks'] for k,v in b['tasks'].items()),'main_ticks':b['tasks'][str(s.process.pid)]['ticks']-a['tasks'][str(s.process.pid)]['ticks'],'main_switches':b['tasks'][str(s.process.pid)]['voluntary_ctxt_switches']-a['tasks'][str(s.process.pid)]['voluntary_ctxt_switches'],'io_delta':{k:b['io'][k]-a['io'][k] for k in a['io']},'property_updates':properties,'redraws_late':len(span),'report_sha256':digest(s.report),'payload_peak':d.get('cpu_payload_peak',d.get('cpu_payload_bytes')),'frame_count':len(d['frames'])}
  rows.append(row);(root/'summary.json').write_text(json.dumps({'binary_sha256':digest(binary),'rows':rows,'scope':'six untimed native windows; 5s external observation after verified quiet main-thread and I/O over one second; raw per-task deltas retained'},indent=2)+'\n');print(name,row,flush=True)
  assert row['main_ticks']==0 and row['main_switches']==0 and not any(row['io_delta'].values()) and properties==0 and not span
 finally:
  if spy:spy.terminate();spy.wait(timeout=3)
  s.kill()
