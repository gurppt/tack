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
args=parser.parse_args()
if os.environ.get('DISPLAY') in (None,':0',':0.0'):parser.error('isolated X11 required')
root=args.output.resolve();root.mkdir(parents=True,exist_ok=False)
binary=args.binary.resolve();fixtures=args.fixtures.resolve()
env=dict(os.environ,TACK_PROFILE_DIR=str(root/'profile')); rows=[]
for name,args in [('empty',['new',root/'empty.tack']),('small',['open',fixtures/'small.tack']),('1k',['open',fixtures/'images-1k.tack']),('5k',['open',fixtures/'images-5k.tack']),('dense-potato',['open',fixtures/'images-5k.tack','--dense-view','--potato']),('overloaded-potato',['open',fixtures/'dense-highlod.tack','--potato'])]:
 s=Session(binary,root,name,args,env)
 spy=None
 try:
  time.sleep(4)
  lf=(root/f'{name}-properties.log').open('w');spy=subprocess.Popen(['xprop','-spy','-id',s.window,'WM_NAME','_NET_WM_NAME'],stdout=lf,stderr=subprocess.STDOUT);time.sleep(.2)
  lines=len((root/f'{name}-properties.log').read_text().splitlines())
  a=observe(s.process.pid);time.sleep(5);b=observe(s.process.pid)
  properties=len((root/f'{name}-properties.log').read_text().splitlines())-lines
  spy.terminate();spy.wait(timeout=3);spy=None;lf.close()
  s.close();d=json.loads(s.report.read_text())
  span=[f for f in d['frames'] if d['native_startup_ms']+4200<=f['elapsed_ms']<=d['native_startup_ms']+9000]
  row={'name':name,'rss_bytes':b['rss_bytes'],'threads':len(b['tasks']),'cpu_ticks_5s':sum(v['ticks']-a['tasks'].get(k,v)['ticks'] for k,v in b['tasks'].items()),'main_ticks':b['tasks'][str(s.process.pid)]['ticks']-a['tasks'][str(s.process.pid)]['ticks'],'main_switches':b['tasks'][str(s.process.pid)]['voluntary_ctxt_switches']-a['tasks'][str(s.process.pid)]['voluntary_ctxt_switches'],'io_delta':{k:b['io'][k]-a['io'][k] for k in a['io']},'property_updates':properties,'redraws_late':len(span),'report_sha256':digest(s.report),'payload_peak':d['cpu_payload_peak'],'frame_count':len(d['frames'])}
  rows.append(row);(root/'summary.json').write_text(json.dumps({'binary_sha256':digest(binary),'rows':rows,'scope':'six untimed native windows; 5s external observation after four seconds settling'},indent=2)+'\n');print(name,row,flush=True)
  assert row['main_ticks']==0 and row['main_switches']==0 and not any(row['io_delta'].values()) and properties==0 and not span
 finally:
  if spy:spy.terminate();spy.wait(timeout=3)
  s.kill()
