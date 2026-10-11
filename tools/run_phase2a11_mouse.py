#!/usr/bin/env python3
"""Production toy on an owned X11 display; native cursor readback and bounded receipts."""
import argparse,json,os,re,signal,subprocess,time
from pathlib import Path
from PIL import Image
from run_local_production import Session,wait
from run_phase2a8_foundation import Cursor,expected
from run_phase2a_local_regression import snapshot,delta
from run_native_image_checks import command

def main():
 p=argparse.ArgumentParser(description=__doc__)
 p.add_argument('--binary',type=Path,required=True)
 p.add_argument('--profile',type=Path,required=True)
 p.add_argument('--output',type=Path,required=True)
 p.add_argument('--hardware',action='store_true')
 a=p.parse_args();root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
 repo=Path(__file__).resolve().parents[1];binary=a.binary.resolve();xr=repo/'target/native/tools/xvfb-root'
 if Path('/tmp/.X11-unix/X129').exists():raise RuntimeError('Display 129 occupied; refusing')
 env=dict(os.environ,DISPLAY=':129',TACK_NATIVE_NO_WM='1',TACK_NATIVE_DIAGNOSTICS='1',WINIT_X11_SCALE_FACTOR='1',LP_NUM_THREADS='2',LD_LIBRARY_PATH=str(xr/'usr/lib/x86_64-linux-gnu'),WGPU_BACKEND='vulkan',XDG_CONFIG_HOME=str(root/'wm'),DBUS_SESSION_BUS_ADDRESS='',GSK_RENDERER='cairo')
 if not a.hardware:env['VK_DRIVER_FILES']='/usr/share/vulkan/icd.d/lvp_icd.json'
 else:env.pop('VK_DRIVER_FILES',None);env.pop('VK_ICD_FILENAMES',None)
 os.environ.update(env);checks=[];runs=[];s=None;cursor=None;wm=None
 log=(root/'display.log').open('w');xvfb=subprocess.Popen([str(xr/'usr/bin/Xvfb'),':129','-screen','0','1400x1000x24','-nolisten','tcp','-noreset'],env=env,stdout=log,stderr=subprocess.STDOUT)
 def record(name,ok,detail=None):
  checks.append(dict(name=name,observed=bool(ok),detail=detail));(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
  print(name,ok,flush=True)
  if not ok:raise AssertionError(name)
 def move(x,y):command('xdotool','mousemove','--window',s.window,str(x),str(y))
 def phase():
  match=re.search(r'GAME (\w+) (\d+)',s.title());return (match.group(1),int(match.group(2))) if match else ('Inactive',0)
 def invisible():return not any(value>>24 for value in cursor.pixels()['pixels'])
 def measure(label,seconds):
  before=snapshot(s.process.pid);start=time.monotonic();time.sleep(seconds);after=snapshot(s.process.pid)
  value=delta(before,after,time.monotonic()-start);value['label']=label;value['main_ticks']=after['tasks'][str(s.process.pid)]['ticks']-before['tasks'][str(s.process.pid)]['ticks'];runs.append(value)
  return value
 try:
  wait(lambda:subprocess.run(['xdpyinfo'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'owned display')
  wm=subprocess.Popen(['dbus-run-session','--','xfwm4','--compositor=off','--sm-client-disable'],env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  wait(lambda:'_NET_SUPPORTING_WM_CHECK(WINDOW)' in command('xprop','-root','_NET_SUPPORTING_WM_CHECK'),'owned WM')
  cursor=Cursor();source=root/'tiny.png';Image.new('RGB',(32,32),(40,60,80)).save(source)
  board=root/'board.tack';subprocess.run([str(binary),'create',str(board),'--linked',str(source)],env=env,check=True,capture_output=True)
  settings=json.loads(a.profile.read_text());settings.update(recent=[],local_views=[],grid=False,status_bar=False,native_title_bar=False,always_on_top=False)
  settings['toolbar']['placement']='Hidden'
  settings['keymap']=[b for b in settings['keymap'] if b['control']!={'LogicalKey':{'Named':'F1'}}]
  settings['keymap'].append(dict(action='SelectTool(Mouse)',control={'LogicalKey':{'Named':'F1'}},modifiers=0,modifier_match='Exact',trigger='Press'))
  # Use the canonical semantic ID from the existing profile format.
  settings['keymap'][-1]['action']='SelectTool(Mouse)'
  profile=root/'profile';profile.mkdir();(profile/'preferences.json').write_text(json.dumps(settings));env['TACK_PROFILE_DIR']=str(profile)
  for size in ['800x600','1024x768']:
   env['TACK_TEST_WINDOW_SIZE']=size;s=Session(binary,root,'game-'+size,['open',board],env);time.sleep(.8);move(400,300);time.sleep(.3)
   base=measure(size+' inactive',2)
   record(size+' inactive zero CPU ticks and write IO',base['ticks']==0 and base['io_delta']['write_bytes']==0,base)
   s.key('F1');wait(lambda:phase()[0]=='SpawnDelay','asset install');record(size+' cheese custom cursor',cursor.pixels()==expected((binary.parent/'gfx/mouse-game/cheese_cursor.png') if (binary.parent/'gfx/mouse-game/cheese_cursor.png').exists() else repo/'gfx/mouse-game/cheese_cursor.png',[14,14],2))
   wait(lambda:phase()[0]=='Run','one-shot spawn',seconds=5)
   before=snapshot(s.process.pid);started=time.monotonic()
   for x in [650,200,650,200,650,200,400]:move(x,300);time.sleep(.25)
   chase=delta(before,snapshot(s.process.pid),time.monotonic()-started);chase['label']=size+' chase';runs.append(chase)
   wait(lambda:phase()[0]=='Idle','spring settles',seconds=10);measure(size+' active idle',1)
   wait(lambda:phase()[0]=='Attack','qualified capture',seconds=6)
   observed={};shots=set();started=time.monotonic();post=None
   while time.monotonic()-started<10:
    state,frame=phase();hidden=invisible();observed.setdefault(state,{})[frame]=hidden
    if state=='Attack' and frame>=11:move(720,430)
    if (state,frame) in [('Attack',10),('Attack',11),('Attack',14),('Attack',15),('Munch',1),('Munch',2),('Explosion',1),('Explosion',11),('PostExplosionWait',1)] and (state,frame) not in shots:
     time.sleep(.025);s.shot(state+'-'+str(frame));shots.add((state,frame))
    if state=='PostExplosionWait' and post is None:post=time.monotonic()
    if state=='Inactive':break
    time.sleep(.008)
   record(size+' frame 10 cheese visible',observed.get('Attack',{}).get(10) is False,observed)
   record(size+' frame 11 and subsequent cursor fully hidden',observed.get('Attack',{}).get(11) is True and all(v for stage in ['Munch','Explosion','PostExplosionWait'] for v in observed.get(stage,{}).values()))
   record(size+' returns normal tool after wait',phase()[0]=='Inactive' and not invisible() and 'Pointer' in s.title())
   time.sleep(.5);settled=measure(size+' completed inactive',2);record(size+' finished zero main-thread CPU ticks and writes',settled['main_ticks']==0 and settled['io_delta']['write_bytes']==0,settled)
   s.close();receipt=json.loads(s.report.read_text());trace=receipt['mouse']['trace'];attack=[r for r in trace if r['state']=='Attack'];explosion=[r for r in trace if r['state']=='Explosion']
   record(size+' all 15 attack and 12 explosion frames presented', [r['frame'] for r in attack]==list(range(1,16)) and [r['frame'] for r in explosion]==list(range(1,13)) and all(r.get('presented') for r in attack+explosion))
   landing=attack[10]['at'];record(size+' one advance and stable final anchors',all(r['advances']==1 and r['at']==landing for r in trace if r['state'] in ['Munch','Explosion','PostExplosionWait']))
   postrow=next(r for r in trace if r['state']=='PostExplosionWait');finished=next(r for r in trace if r['state']=='Finished');record(size+' full 1 second hidden wait',finished['ms']-postrow['ms']>=1000,finished['ms']-postrow['ms'])
   record(size+' releases assets, game and deadlines',not receipt['mouse']['game_active'] and receipt['mouse']['artwork_bytes']==0 and not receipt['mouse']['deadline_active'])
   if size=='800x600':
    # Separate owned run: cancellation while hidden, then focus loss without toy restart.
    s=Session(binary,root,'cancel',['open',board],env);time.sleep(.8);move(400,300);s.key('F1');wait(lambda:phase()[0]=='SpawnDelay','cancel asset install');s.key('Escape');record('Escape during SpawnDelay cancels',phase()[0]=='Inactive' and not invisible())
    s.key('F1');wait(lambda:(phase()[0]=='Attack' and phase()[1]>=11) or phase()[0] in ('Munch','Explosion','PostExplosionWait'),'hidden attack',seconds=22);record('hidden before Escape',invisible());s.key('Escape');record('Escape while hidden restores pointer',phase()[0]=='Inactive' and not invisible())
    s.key('F1');wait(lambda:(phase()[0]=='Attack' and phase()[1]>=11) or phase()[0] in ('Munch','Explosion','PostExplosionWait'),'hidden focus-loss stage',seconds=22)
    rootid=re.search(r'Window id: (0x[0-9a-f]+)',command('xwininfo','-root')).group(1);command('xdotool','windowfocus',rootid);time.sleep(.2);s.focus();record('Focus loss cancels and cannot restart',phase()[0]=='Inactive' and not invisible());s.close()
  (root/'measurements.json').write_text(json.dumps(runs,indent=2)+'\n')
  print(json.dumps(dict(checks=len(checks),passed=all(x['observed'] for x in checks))),flush=True)
 finally:
  if s and s.process.poll() is None:
   try:s.close(discard=True)
   except Exception:s.kill()
  if cursor:cursor.close()
  if wm:
   os.killpg(wm.pid,signal.SIGTERM);wm.wait(timeout=5)
  xvfb.terminate();xvfb.wait(timeout=5);log.close()
if __name__=='__main__':main()
