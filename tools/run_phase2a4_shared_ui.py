#!/usr/bin/env python3
"""Owned-display Share/Join/lifecycle + primitive toolbar witness; no manual server."""
import argparse,json,os,subprocess,time,socket
from pathlib import Path
from PIL import Image
from run_local_production import Session,wait
from run_native_image_checks import command
from run_idle import close_owned_window
from run_image_interaction import digest
from run_phase1l_local import descendants
from run_phase2a_native import snapshot as authority_snapshot,native_revision
from run_phase2a_local_regression import snapshot as process_snapshot
class Window:
 def __init__(self,window,root,name):self.window=window;self.root=root;self.name=name
 def title(self):return command('xdotool','getwindowname',self.window)
 def focus(self):command('xdotool','windowraise',self.window);command('xdotool','windowfocus','--sync',self.window);command('xdotool','keyup','ctrl','alt','shift','super')
 def key(self,*keys):self.focus();command('xdotool','key',*keys);time.sleep(.15)
 def shot(self,label):self.focus();subprocess.run(['import','-window',self.window,str(self.root/f'{self.name}-{label}.png')],check=True,timeout=4)
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',type=Path,required=True);p.add_argument('--profile',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 if os.environ.get('DISPLAY') in (None,':0',':0.0') or os.environ.get('TACK_NATIVE_NO_WM')!='1':p.error('Owned isolated display required')
 with socket.socket() as probe:probe.bind(('0.0.0.0',7337)) # Fail safely if another host already owns the default port.
 root=a.output.resolve();root.mkdir(parents=True,exist_ok=False);binary=a.binary.resolve();checks=[];sessions=[];windows=[]
 env=dict(os.environ,TACK_NATIVE_DIAGNOSTICS="1",TACK_TEST_WINDOW_SIZE='800x600',WINIT_X11_SCALE_FACTOR='1',XDG_DATA_HOME=str(root/'data'),XDG_CONFIG_HOME=str(root/'config'),GSETTINGS_BACKEND='memory')
 profile=json.loads(a.profile.read_text());profile.update(recent=[],ui_scale=1)
 remaps={'F6':'ShareBoard','F7':'StopSharing','F8':'EditToolbar','F9':'JoinSharedBoard','F10':'ToggleToolbar','F12':'CopySharedBoardAddress'}
 profile['keymap']=[b for b in profile['keymap'] if b['action'] not in remaps.values() and b['control'] not in [{'LogicalKey':{'Named':k}} for k in remaps]]
 for key,action in remaps.items():profile['keymap'].append(dict(action=action,control={'LogicalKey':{'Named':key}},modifiers=0,modifier_match='Exact',trigger='Press'))
 def record(name,ok,detail=None):
  checks.append(dict(name=name,observed=bool(ok),detail=detail));(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
  if not ok:raise AssertionError(name)
 def cli(*args):
  r=subprocess.run([str(binary),*map(str,args)],env=env,capture_output=True,text=True,timeout=30)
  if r.returncode:raise AssertionError(r.stderr)
  return json.loads(r.stdout)
 def session(name,args,reuse=None):
  local=dict(env,TACK_PROFILE_DIR=str(root/((reuse or name)+'-profile')))
  if reuse is None:
   Path(local['TACK_PROFILE_DIR']).mkdir();(Path(local['TACK_PROFILE_DIR'])/'preferences.json').write_text(json.dumps(profile))
  s=Session(binary,root,name,args,local);sessions.append(s);return s
 def child_window(parent,name):
  def found():
   for pid in descendants(parent.process.pid):
    r=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(pid)],capture_output=True,text=True)
    for win in r.stdout.splitlines():
     title=command('xdotool','getwindowname',win)
     if ' · Connected ·' in title:return Window(win,root,name)
   return None
  w=wait(found,'owned shared window connected',seconds=20);windows.append(w);command('xdotool','windowsize',w.window,'800','600');time.sleep(.3);return w
 def pid(w):return int(command('xdotool','getwindowpid',w.window))
 def hosted_server(w):return wait(lambda:next((p for p in descendants(pid(w)) if Path(f'/proc/{p}/comm').read_text().strip()=='tack-server'),None),'owned managed server')
 try:
  img=root/'large.png';Image.new('RGB',(4096,2160),(215,37,68)).save(img);local=root/'artist.tack';cli('create',local,'--embedded',img)
  before_hash=digest(local);before_id=cli('inspect',local)['document_id'];s=session('artist-a',['open',local]);time.sleep(1)
  record('ordinary local starts without IP sockets',not any(x['kind']!='unix' for x in process_snapshot(s.process.pid)['sockets']))
  s.shot('toolbar-default');s.key('F10');s.shot('toolbar-hidden');s.key('F10');s.key('F8');s.shot('toolbar-editor');s.key('Escape')
  # Full GUI share path and real native Save picker; server starts only inside owned hosted child.
  s.key('F6');s.shot('share-choice');s.key('Return');s.shot('share-confirm');target=root/'artist-shared.tack';s.picker('Return',target)
  host=child_window(s,'host');server=hosted_server(host);meta=json.loads(Path(str(target)+'.sharing.json').read_text());uri=meta['invite'];address=uri[7:].split('/')[0];board=meta['board']
  record('GUI creates named independent shared incarnation',target.name=='artist-shared.tack' and board!=before_id and digest(local)==before_hash,meta)
  host.shot('online-panel');host.key('Return');time.sleep(.3)
  copied=subprocess.run(['xclip','-selection','clipboard','-out'],capture_output=True,text=True,timeout=3)
  record('Copy Invite button returns canonical invite',copied.returncode==0 and copied.stdout==uri,copied.stdout)
  host.key('Escape');host.shot('online-status')
  b=session('artist-b',[]);b.key('F9');b.key('ctrl+v');time.sleep(.5);b.key('Return');partner=child_window(b,'partner');partner.key('Escape');partner.shot('joined')
  record('second GUI joins invitation without CLI server',native_revision(partner)==0)
  host.key('ctrl+a','alt+shift+h');wait(lambda:native_revision(partner)==1 and native_revision(host)==1,'converged semantic edit')
  record('existing shared-supported edit converges',authority_snapshot(address,board)['revision']==1)
  host_title=host.title();partner.focus();command('xdotool','mousemove','--window',partner.window,'400','300');command('xdotool','click','4');time.sleep(.2);record('partner camera independent',host.title()==host_title)
  record('local original alongside remains untouched',digest(local)==before_hash)
  host.key('F7');wait(lambda:not Path(f'/proc/{server}').exists(),'server reaped after Stop',seconds=15);wait(lambda:'Disconnected' in partner.title() or 'ServerUnavailable' in partner.title(),'partner notices offline')
  host.shot('offline-reopen-choice');host.key('Escape');host.shot('offline-status');saved_hash=digest(target);host.key('ctrl+a','alt+shift+h');time.sleep(.3)
  record('offline copy is readonly and no merge authority',digest(target)==saved_hash and not any(x['kind']!='unix' for x in process_snapshot(pid(host))['sockets']))
  record('Stop leaves no owned server',not Path(f'/proc/{server}').exists())
  host_pid=pid(host);close_owned_window(host.window);windows.remove(host);wait(lambda:not Path(f'/proc/{host_pid}').exists(),'host window closes',seconds=8)
  reopened=session('reopened',['open',target],reuse='artist-a');reopened.shot('reopen-choice');reopened.key('Down','Return');wait(lambda:' · Connected ·' in reopened.title(),'Put Online reopens same authority',seconds=15);host2=Window(reopened.window,root,'host2');server2=hosted_server(host2)
  meta2=json.loads(Path(str(target)+'.sharing.json').read_text());record('Put Online preserves old invitation identity',meta2==meta and authority_snapshot(address,board)['revision']==1)
  partner.key('F5');wait(lambda:native_revision(partner)==1,'previous invite reconnects');record('old invitation joins after host restart',native_revision(partner)==1)
  host2.key('Escape');host2.shot('reonline-status');close_owned_window(host2.window);wait(lambda:reopened.process.poll() is not None,'host close waits for shutdown',seconds=15);record('host close reaps managed server',not Path(f'/proc/{server2}').exists())
  partner.key('Escape');close_owned_window(partner.window);windows.remove(partner)
  s.focus();s.key('Escape');s.close();b.focus();b.key('Escape');b.close()
  (root/'receipt.json').write_text(json.dumps(dict(binary_sha256=digest(binary),harness_sha256=digest(__file__),scope='Same Linux host, real LAN address, owned X11; no physical two-computer claim',checks=checks),indent=2)+'\n')
 finally:
  for w in windows:
   try:close_owned_window(w.window)
   except (subprocess.SubprocessError,OSError):pass
  for s in sessions:s.kill()
 print(root/'receipt.json')
if __name__=='__main__':main()
