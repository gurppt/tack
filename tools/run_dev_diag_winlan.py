#!/usr/bin/env python3
"""Owned local storage and isolated X11 LOD receipts for DEV-DIAG-WINLAN-01."""
import argparse, json, os, shutil, socket, subprocess, time
from pathlib import Path
from run_local_production import Session, wait
from run_phase2a_local_regression import snapshot, delta

def main():
 p=argparse.ArgumentParser();p.add_argument('--package',type=Path,required=True);p.add_argument('--fixtures',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--save-probe',type=Path,required=True);p.add_argument('--profile-template',type=Path,required=True);p.add_argument('--native-only',action='store_true');a=p.parse_args()
 root=a.output.resolve();root.mkdir(exist_ok=False);fixtures=a.fixtures.resolve();package=a.package.resolve();binary=package/'tack';repo=Path(__file__).resolve().parents[1]
 assert (fixtures/'OWNER_DIAG_FIXTURES').read_text()=='DEV-DIAG-WINLAN-01 disposable fixtures\n'
 if shutil.disk_usage(root).free<10*1024**3:raise RuntimeError('10 GiB reserve')
 env=dict(os.environ,TACK_PROFILE_DIR=str(root/'profile'),TACK_DEV_DIAGNOSTICS='1',TACK_DIAGNOSTICS_DIR=str(root/'logs'));env.pop('TACK_ICON_DIR',None)
 boards={};saves={}
 for fmt in ([] if a.native_only else ['jpeg','png']):
  sources=sorted(fixtures.glob('*.jpg' if fmt=='jpeg' else '*.png'))[:2]
  for mode in ['linked','embedded']:
   board=fixtures/f'{fmt}-{mode}.tack';boards[fmt,mode]=board
   result=subprocess.run([str(binary),'create',str(board),'--'+mode,*map(str,sources)],env=env,capture_output=True,text=True,timeout=90,check=True)
   (root/f'create-{fmt}-{mode}.json').write_text(result.stdout)
   for diagnostics in ['off','on']:
    runenv=dict(env,TACK_DEV_DIAGNOSTICS='1' if diagnostics=='on' else '0')
    result=subprocess.run([str(a.save_probe.resolve()),str(board)],env=runenv,capture_output=True,text=True,timeout=90,check=True)
    saves[f'{fmt}-{mode}-{diagnostics}']=json.loads(result.stdout)
 (root/'save-results.json').write_text(json.dumps(saves,indent=2)+'\n')
 # Never sends traffic to the user's 7337 listener. Every root/process below is ours.
 with socket.socket() as reservation:reservation.bind(('127.0.0.1',0));port=reservation.getsockname()[1]
 server=subprocess.Popen([str(package/'tack-server'),'--root',str(root/'server-free'),'--listen',f'127.0.0.1:{port}'],stdin=subprocess.PIPE,stdout=subprocess.DEVNULL,stderr=(root/'server.log').open('w'),env=env)
 try:
  def bound():
   with socket.socket() as s:s.settimeout(.1);return s.connect_ex(('127.0.0.1',port))==0
  wait(bound,'owned server bound')
  conflict=subprocess.run([str(package/'tack-server'),'--root',str(root/'server-conflict'),'--listen',f'127.0.0.1:{port}'],env=env,capture_output=True,text=True,timeout=10)
  lock=subprocess.run([str(package/'tack-server'),'--root',str(root/'server-free'),'--listen','127.0.0.1:0'],env=env,capture_output=True,text=True,timeout=10)
  assert conflict.returncode==1 and lock.returncode==1
  (root/'server-results.json').write_text(json.dumps(dict(port=port,conflict_exit=conflict.returncode,lock_exit=lock.returncode,scope='owned port/root only, no user server mutations'),indent=2)+'\n')
 finally:server.terminate();server.wait(timeout=5)
 xr=repo/'target/native/tools/xvfb-root';display=':163';assert not Path('/tmp/.X163-lock').exists()
 env.update(DISPLAY=display,TACK_NATIVE_NO_WM='1',WINIT_X11_SCALE_FACTOR='1',LP_NUM_THREADS='2',TACK_TEST_WINDOW_SIZE='800x600',LD_LIBRARY_PATH=str(xr/'usr/lib/x86_64-linux-gnu'),VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',WGPU_BACKEND='vulkan')
 os.environ.update(env)
 profile=root/'profile';profile.mkdir();settings=json.loads(a.profile_template.read_text());settings.update(recent=[],local_views=[])
 settings['keymap']=[b for b in settings['keymap'] if b['action']!='ActualSize' and b['control']!={'LogicalKey':{'Named':'F5'}}]
 settings['keymap'].append(dict(action='ActualSize',control={'LogicalKey':{'Named':'F5'}},modifiers=0,modifier_match='Exact',trigger='Press'))
 (profile/'preferences.json').write_text(json.dumps(settings))
 xlog=(root/'xvfb.log').open('w');x=subprocess.Popen([str(xr/'usr/bin/Xvfb'),display,'-screen','0','1024x768x24','-nolisten','tcp','-noreset'],env=env,stdout=xlog,stderr=subprocess.STDOUT);session=None
 receipts=[]
 try:
  wait(lambda:subprocess.run(['xdpyinfo'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'owned X display')
  for fmt,tiles,diag in [('jpeg',False,True),('png',False,True),('png',True,True),('jpeg',False,False)]:
   name=f'{fmt}-tiles-{tiles}-diag-{diag}';runenv=dict(env,TACK_DEV_DIAGNOSTICS='1' if diag else '0')
   # ActualSize is a toggle; each case must start with virgin display geometry.
   case_board=fixtures/(name+'-virgin.tack')
   sources=sorted(fixtures.glob('*.jpg' if fmt=='jpeg' else '*.png'))[:2]
   subprocess.run([str(binary),'create',str(case_board),'--embedded',*map(str,sources)],env=runenv,capture_output=True,text=True,check=True,timeout=90)
   args=['open',case_board,'--lod-debug']+(['--huge-tiles'] if tiles else [])
   session=Session(binary,root,name,args,runenv);time.sleep(2);session.key('Escape','ctrl+a','F5');session.save(case_board);time.sleep(6)
   before=snapshot(session.process.pid);time.sleep(3);after=snapshot(session.process.pid);idle=delta(before,after,3)
   session.close();report=json.loads(session.report.read_text());session=None
   fs=report['frames'];receipts.append(dict(name=name,idle=idle,frames=len(fs),errors=report['errors'],max_tiles_requested=max(f['tiles_requested'] for f in fs),max_tiles_ready=max(f['tiles_ready'] for f in fs),last_lod=next((f['lod_trace'] for f in reversed(fs) if f.get('lod_trace')),None),max_gpu_bytes=max(f['gpu_bytes'] for f in fs),max_cpu_payload=max(f['supply']['cpu_bytes'] for f in fs if f['supply'])))
   (root/'lod-results.json').write_text(json.dumps(receipts,indent=2)+'\n')
  # Same isolated software renderer, blank-board on/off attribution only.
  for diag in [False,True]:
   name=f'blank-diag-{diag}';session=Session(binary,root,name,[],dict(env,TACK_DEV_DIAGNOSTICS='1' if diag else '0'));time.sleep(2)
   before=snapshot(session.process.pid);time.sleep(3);after=snapshot(session.process.pid);idle=delta(before,after,3)
   status=Path(f'/proc/{session.process.pid}/status').read_text();maps=Path(f'/proc/{session.process.pid}/smaps_rollup').read_text()
   (root/f'{name}-memory.txt').write_text(status+maps)
   session.close();session=None;receipts.append(dict(name=name,idle=idle))
  (root/'lod-results.json').write_text(json.dumps(receipts,indent=2)+'\n')
 finally:
  if session:session.kill()
  x.terminate();x.wait(timeout=5);xlog.close()
 print(json.dumps(dict(saves=len(saves),native=len(receipts))))
if __name__=='__main__':main()
