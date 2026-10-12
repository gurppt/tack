#!/usr/bin/env python3
"""Owned private-X11 20-source ordinary baseline. Never drives the real desktop."""
import argparse, hashlib, json, os, random, statistics, subprocess, sys, time
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from run_local_production import Session, wait
from run_idle import observe
p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--profile-template',type=Path,required=True);a=p.parse_args()
root=a.output.resolve();root.mkdir(parents=True,exist_ok=False);binary=a.binary.resolve();repo=Path(__file__).resolve().parents[2]
from PIL import Image
sources=[];fixtures=[]
for i in range(20):
    rng=random.Random(221013+i);im=Image.frombytes('RGB',(600,400),rng.randbytes(600*400*3));path=root/(f'image-{i:02}.'+('jpg' if i<10 else 'png'))
    im.save(path,**({'quality':85,'progressive':False} if i<10 else {'compress_level':1}));sources.append(path);fixtures.append(dict(file=path.name,seed=221013+i,size=[600,400],bytes=path.stat().st_size,sha256=hashlib.file_digest(path.open('rb'),'sha256').hexdigest(),layout='8-bit RGB noninterlaced/baseline'))
(root/'fixtures.json').write_text(json.dumps(fixtures,indent=2)+'\n')
xr=repo/'target/native/tools/xvfb-root';display=next(f':{i}' for i in range(166,180) if not Path(f'/tmp/.X{i}-lock').exists())
env=dict(os.environ,DISPLAY=display,TACK_NATIVE_NO_WM='1',WINIT_X11_SCALE_FACTOR='1',LP_NUM_THREADS='2',TACK_TEST_WINDOW_SIZE='800x600',LD_LIBRARY_PATH=str(xr/'usr/lib/x86_64-linux-gnu'),VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',WGPU_BACKEND='vulkan',TACK_DEV_DIAGNOSTICS='0',TACK_PROFILE_DIR=str(root/'profile'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'))
os.environ.update(env);profile=root/'profile';profile.mkdir();settings=json.loads(a.profile_template.read_text());settings.update(recent=[],local_views=[]);(profile/'preferences.json').write_text(json.dumps(settings))
board=root/'ordinary.tack';subprocess.run([str(binary),'create',str(board),'--embedded',*map(str,sources)],env=env,capture_output=True,check=True,timeout=60)
xlog=(root/'xvfb.log').open('w');x=subprocess.Popen([str(xr/'usr/bin/Xvfb'),display,'-screen','0','1024x768x24','-nolisten','tcp','-noreset'],env=env,stdout=xlog,stderr=subprocess.STDOUT);session=None;receipts=[]
try:
    wait(lambda:subprocess.run(['xdpyinfo'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'private X display')
    for run in range(3):
        name=f'run-{run}';session=Session(binary,root,name,['open',board,'--lod-debug'],env);mapped=time.monotonic()-session.started;time.sleep(3)
        before=observe(session.process.pid);smaps=Path(f'/proc/{session.process.pid}/smaps_rollup').read_text();time.sleep(3);after=observe(session.process.pid)
        idle=dict(seconds=3,cpu_ticks=sum(v['ticks'] for v in after['tasks'].values())-sum(v['ticks'] for v in before['tasks'].values()),io={k:after['io'][k]-before['io'][k] for k in after['io']},threads=len(after['tasks']),thread_names=[t['name'] for t in after['tasks'].values()],rss=after['rss_bytes'])
        (root/f'{name}-smaps.txt').write_text(smaps)
        # Isolated display only, active camera changes cause actual renderer frames.
        session.key('ctrl+a');subprocess.run(['xdotool','mousemove','--window',session.window,'400','300'],env=env,check=True)
        subprocess.run(['xdotool','click','--repeat','16','--delay','70','4'],env=env,check=True);subprocess.run(['xdotool','click','--repeat','16','--delay','70','5'],env=env,check=True)
        session.key('Escape');start=time.monotonic();session.save(board);save_elapsed=time.monotonic()-start
        session.close();report=json.loads(session.report.read_text());session=None;frames=report['frames'];times=[f['callback_ms'] for f in frames if 'callback_ms' in f]
        if not times:
            times=[f['frame_cpu_ms'] for f in frames if 'frame_cpu_ms' in f]
        def pct(q):return sorted(times)[min(len(times)-1,int((len(times)-1)*q))] if times else None
        receipts.append(dict(run=run,startup_to_mapped_s=mapped,filesystem_cache='warm/uncontrolled; no global cache evictions',unique_sources=20,objects=20,idle=idle,save_ack_wall_s=save_elapsed,frame_count=len(frames),frame_fields=list(frames[0]) if frames else [],frame_p50_ms=pct(.5),frame_p95_ms=pct(.95),frame_p99_ms=pct(.99),errors=report['errors'],max_gpu_bytes=max((f['gpu_bytes'] for f in frames),default=0),pss_bytes=next(int(l.split()[1])*1024 for l in smaps.splitlines() if l.startswith('Pss:'))))
        (root/'results.json').write_text(json.dumps(receipts,indent=2)+'\n')
finally:
    if session:session.kill()
    x.terminate();x.wait(timeout=5);xlog.close()
print(json.dumps({'runs':len(receipts),'errors':[r['errors'] for r in receipts]}))
