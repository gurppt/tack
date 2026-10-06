#!/usr/bin/env python3
"""Actual Dolphin clipboard plus owned-window screenshot; isolated X11 only."""
import argparse
import os,json,subprocess,time,signal
from pathlib import Path
from PIL import Image
from run_local_production import Session,wait
from run_native_image_checks import command
from run_image_interaction import digest
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--probe', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
if os.environ.get('DISPLAY') in (None, ':0', ':0.0') or os.environ.get('TACK_NATIVE_NO_WM') != '1':
 parser.error('requires explicitly owned isolated X11 display')
binary, diagnostic = args.binary.resolve(), args.probe.resolve()
helper = command('which', 'xclip').strip()
root=args.output.resolve();root.mkdir(parents=True,exist_ok=False)
images=root/'images';images.mkdir()
Image.new('RGB',(80,60),(170,80,60)).save(images/'a.png');Image.new('RGB',(70,90),(50,90,180)).save(images/'b.jpg')
env=dict(os.environ,TACK_NATIVE_NO_WM='1',TACK_PROFILE_DIR=str(root/'profile'),XDG_DATA_HOME=str(root/'data'),XDG_CONFIG_HOME=str(root/'config'),XDG_CACHE_HOME=str(root/'cache'),QT_QPA_PLATFORM='xcb',QT_QUICK_BACKEND='software',QT_ACCESSIBILITY='0',LIBGL_ALWAYS_SOFTWARE='1',TACK_TEST_WINDOW_SIZE='800x600')
s=Session(binary,root,'app',['new',root/'board.tack'],env)
processes=[];cases=[]
def probe(name):
 r=subprocess.run([str(diagnostic),str(root/'probe')],env=env,capture_output=True,text=True,check=True,timeout=15)
 value=json.loads(r.stdout);cases.append(dict(case=name,normalized=value,trace=r.stderr,display=env['DISPLAY'],session='X11',binary_sha256=digest(binary)))
 (root/'receipt.json').write_text(json.dumps(cases,indent=2)+'\n');return value
try:
 log=(root/'flameshot.log').open('w')
 flame=subprocess.Popen(['dbus-run-session','--','flameshot','full','-c','--region','800x600+0+0'],env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);processes.append(flame)
 time.sleep(4)
 v=probe('actual Flameshot capture to clipboard')
 if v['kind']!='embedded image':
  # Capture the actual owned native window, with explicit xclip selection ownership.
  capture=root/'actual-screenshot.png'
  subprocess.run(['import','-window',s.window,str(capture)],check=True,env=env,timeout=4)
  owner=subprocess.Popen([helper,'-selection','clipboard','-in','-quiet','-target','image/png'],stdin=subprocess.PIPE,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,env=env,start_new_session=True);processes.append(owner)
  owner.stdin.write(capture.read_bytes());owner.stdin.close();time.sleep(.2)
  v=probe('actual owned window screenshot + xclip selection');assert v['kind']=='embedded image',v
 s.focus();s.key('ctrl+v');wait(lambda:'modified' in s.title() and 'Import ' not in s.title(),'native screenshot imported')
 assert len(s.save(root/'board.tack')['objects'])==1
 cases[-1]['imported_total']=1
 (root/'receipt.json').write_text(json.dumps(cases,indent=2)+'\n')
 log=(root/'dolphin.log').open('w')
 dolphin=subprocess.Popen(['dbus-run-session','--','dolphin','--new-window','--select',str(images/'a.png')],env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);processes.append(dolphin)
 def window():
  r=subprocess.run(['xdotool','search','--onlyvisible','--class','dolphin'],env=env,capture_output=True,text=True,timeout=3)
  return r.stdout.splitlines()[-1] if r.stdout.strip() else None
 w=wait(window,'owned Dolphin window',seconds=15)
 command('xdotool','windowfocus','--sync',w);time.sleep(.3);command('xdotool','key','ctrl+c');time.sleep(.3)
 v=probe('actual Dolphin copied single PNG');assert v['kind']=='local files' and v['count']==1,v
 s.focus();s.key('ctrl+v');wait(lambda:'modified' in s.title() and 'Import ' not in s.title(),'Dolphin single imported');assert len(s.save(root/'board.tack')['objects'])==2;cases[-1]['imported_total']=2
 command('xdotool','windowfocus','--sync',w);command('xdotool','key','End','ctrl+c');time.sleep(.3)
 v=probe('actual Dolphin copied single JPEG');assert v['kind']=='local files' and v['count']==1,v
 s.focus();s.key('ctrl+v');wait(lambda:'modified' in s.title() and 'Import ' not in s.title(),'Dolphin JPEG imported');assert len(s.save(root/'board.tack')['objects'])==3;cases[-1]['imported_total']=3
 command('xdotool','windowfocus','--sync',w);command('xdotool','key','ctrl+a','ctrl+c');time.sleep(.3)
 v=probe('actual Dolphin multiple images');assert v['kind']=='local files' and v['count']==2,v
 s.focus();s.key('ctrl+v');wait(lambda:'modified' in s.title() and 'Import ' not in s.title(),'Dolphin multiple imported');assert len(s.save(root/'board.tack')['objects'])==5;cases[-1]['imported_total']=5
 s.shot('actual-collages');s.close()
 print(json.dumps(cases,indent=2))
finally:
 s.kill()
 for p in processes:
  try:os.killpg(p.pid,signal.SIGTERM)
  except ProcessLookupError:pass
  try:p.wait(timeout=3)
  except subprocess.TimeoutExpired:
   os.killpg(p.pid,signal.SIGKILL);p.wait(timeout=3)
 for p in (root/'probe').glob('clipboard.*'):p.unlink()
 (root/'receipt.json').write_text(json.dumps(cases,indent=2)+'\n')
