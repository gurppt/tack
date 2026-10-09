#!/usr/bin/env python3
"""Owned X11 daily-use and desktop join witness; never touches a user session."""
import argparse,json,os,re,socket,subprocess,time
from pathlib import Path
from PIL import Image
from run_local_production import Session,wait
from run_native_image_checks import command
from run_image_interaction import digest
from run_idle import close_owned_window
from run_phase2a_local_regression import snapshot,delta

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True);p.add_argument('--server',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True);p.add_argument('--profile',type=Path,required=True)
    a=p.parse_args()
    if os.environ.get('DISPLAY') in (None,':0',':0.0') or os.environ.get('TACK_NATIVE_NO_WM')!='1': p.error('Owned isolated display required')
    root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
    binary=a.binary.resolve();env=dict(os.environ,TACK_NATIVE_DIAGNOSTICS="1",TACK_PROFILE_DIR=str(root/'profile'),TACK_TEST_WINDOW_SIZE='800x600',XDG_DATA_HOME=str(root/'data'),XDG_CONFIG_HOME=str(root/'config'))
    profile=json.loads(a.profile.read_text());profile.update(recent=[],ui_scale=1)
    remaps={'F6':'AddCameraBookmark','F7':'CameraBookmarks','F8':'SourceInfo','F9':'JoinSharedBoard','F12':'CopySharedBoardAddress'}
    profile['keymap']=[b for b in profile['keymap'] if b['action'] not in [*remaps.values(),'DuplicateSelection']]
    for key,action in remaps.items(): profile['keymap'].append(dict(action=action,control={'LogicalKey':{'Named':key}},modifiers=0,modifier_match='Exact',trigger='Press'))
    profile['keymap'].append(dict(action='DuplicateSelection',control={'LogicalKey':{'Character':'d'}},modifiers=2,modifier_match='Exact',trigger='Press'))
    (root/'profile').mkdir();(root/'profile/preferences.json').write_text(json.dumps(profile))
    checks=[];s=None;server=None;child_windows=[]
    def record(name,valid,detail=None):
        checks.append(dict(name=name,observed=bool(valid),detail=detail));(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if not valid: raise AssertionError(name)
    def cli(*args):
        r=subprocess.run([str(binary),*map(str,args)],env=env,capture_output=True,text=True,timeout=30)
        if r.returncode: raise AssertionError(r.stderr)
        return json.loads(r.stdout[r.stdout.index('{'):])
    def inspect(): return cli('inspect',root/'daily.tack')
    def save(): s.save(root/'daily.tack');return inspect()
    def descendants():
        matches=[]
        for proc in Path('/proc').glob('[0-9]*'):
            try:
                status=(proc/'status').read_text();parent=int(re.search(r'^PPid:\s*(\d+)',status,re.M)[1])
                if parent==s.process.pid and 'tack' in (proc/'comm').read_text(): matches.append(int(proc.name))
            except (OSError,TypeError):pass
        return matches
    try:
        image=root/'image.png';Image.new('RGB',(96,96),(220,40,60)).save(image)
        cli('create',root/'daily.tack','--embedded',image)
        s=Session(binary,root,'daily',['open',root/'daily.tack'],env)
        time.sleep(1)
        before=snapshot(s.process.pid);time.sleep(.8);after=snapshot(s.process.pid)
        record('local before join: no IP sockets',not any(x['kind']!='unix' for x in after['sockets']))
        s.key('F6');s.text('Overview');s.key('Return','Escape');bookmarks=save()['bookmarks'];record('add persisted exact bookmark',len(bookmarks)==1 and bookmarks[0]['name']=='Overview',bookmarks)
        s.key('F7','F2');s.text('Close-up');s.key('Return','Escape');record('rename stable ID',save()['bookmarks'][0]['id']==bookmarks[0]['id'] and inspect()['bookmarks'][0]['name']=='Close-up')
        s.key('ctrl+z');record('rename one Undo',save()['bookmarks'][0]['name']=='Overview');s.key('ctrl+shift+z');save()
        command('xdotool','mousemove','--window',s.window,'400','300');command('xdotool','click','--repeat','4','--delay','40','4');time.sleep(.3)
        s.key('F7','Return');s.shot('jump');jump_end=(time.monotonic()-s.started)*1000
        s.key('F7','Delete','Escape');record('delete bookmark',len(save()['bookmarks'])==0);s.key('ctrl+z');record('delete Undo',len(save()['bookmarks'])==1)
        s.key('ctrl+a','ctrl+d');record('duplicate reuses original and asset',save()['objects']==2 and inspect()['assets']==1 and inspect()['embedded']==1)
        s.key('ctrl+z');record('duplicate one Undo',save()['objects']==1);s.key('ctrl+shift+z');record('duplicate Redo',save()['objects']==2)
        command('xdotool','mousemove','--window',s.window,'400','300');command('xdotool','click','1');time.sleep(.2)
        record('information has one selected image', '1 selected' in s.title())
        s.key('F8');s.shot('image-info');time.sleep(.5);s.key('Escape');info_end=(time.monotonic()-s.started)*1000
        baseline=inspect();filehash=digest(root/'daily.tack')
        s.key('F9');s.text('tack://broken/invalid');s.key('Return');s.shot('malformed');record('malformed join keeps document',all(inspect()[k]==baseline[k] for k in ('document_id','objects','assets','sources','embedded','bookmarks')) and not descendants());s.key('Escape')
        with socket.socket() as sock: sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
        bad=f'tack://127.0.0.1:{port}/00000000000000000000000000000001'
        s.key('F9');s.text(bad);s.key('Return');time.sleep(1);s.shot('unavailable');record('failed join cleans child',not descendants());s.key('Escape')
        s.key('F9');s.text(bad);s.key('Return','Escape','F9');s.shot('cancel-reopen');time.sleep(.3);record('cancel leaves no join child',not descendants());s.key('Escape')
        serverlog=(root/'server.log').open('w')
        server=subprocess.Popen([str(a.server.resolve()),'--root',str(root/'server-data'),'--listen','127.0.0.1:0'],stdout=serverlog,stderr=subprocess.STDOUT)
        def address():
            match=re.search(r'listen=(\S+)',(root/'server.log').read_text());return match[1] if match else None
        address=wait(address,'test server listening');published=cli('publish',root/'daily.tack',address);board=published['board']
        uri=f'tack://{address}/{board}'
        s.key('F9');s.text(f'{address} {board}');s.key('Return')
        def joined_window():
            for pid in descendants():
                result=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(pid)],capture_output=True,text=True)
                for window in result.stdout.splitlines():
                    if 'Connected' in command('xdotool','getwindowname',window):return window
            return None
        window=wait(joined_window,'same native backend joined',seconds=12);child_windows.append(window)
        command('xdotool','windowsize',window,'800','600');time.sleep(.2)
        record('desktop join preserves local file',digest(root/'daily.tack')==filehash)
        command('xdotool','windowfocus','--sync',window);command('xdotool','key','F12');time.sleep(.3)
        copied=subprocess.run(['xclip','-selection','clipboard','-out'],capture_output=True,text=True,timeout=3)
        record('canonical shared URI copy round-trips',copied.returncode==0 and copied.stdout==uri,copied.stdout)
        command('xdotool','key','ctrl+a','ctrl+d');time.sleep(.2)
        subprocess.run(['import','-window',window,str(root/'shared-duplicate-disabled.png')],check=True,timeout=4)
        record('shared duplicate remains disabled', '2 selected' in command('xdotool','getwindowname',window))
        command('xdotool','key','Escape');close_owned_window(window);child_windows.remove(window)
        s.focus();s.key('Escape');time.sleep(.3)
        record('join completes without parent IP socket',not any(x['kind']!='unix' for x in snapshot(s.process.pid)['sockets']))
        s.close();report=json.loads(s.report.read_text());near=[f for f in report['frames'] if f['elapsed_ms']<=jump_end];pose=near[-1]
        record('bookmark jump exact camera',pose['camera']==bookmarks[0]['center'] and pose['zoom']==bookmarks[0]['zoom'],dict(actual=[pose['camera'],pose['zoom']],expected=bookmarks[0]))
        before_info=[f for f in report['frames'] if f['elapsed_ms']<info_end-650];after_info=[f for f in report['frames'] if f['elapsed_ms']<=info_end]
        record('info did not admit decode', before_info[-1]['supply']['decode_count']==after_info[-1]['supply']['decode_count'])
        reopened=Session(binary,root,'reopen',['open',root/'daily.tack'],env);reopened.key('F7');reopened.shot('persisted');reopened.key('Escape');reopened.close();reopened.kill()
        record('reopen persisted bookmarks',len(inspect()['bookmarks'])==1)
        (root/'receipt.json').write_text(json.dumps(dict(binary_sha256=digest(binary),harness_sha256=digest(__file__),checks=checks,report=report),indent=2)+'\n')
    finally:
        for window in child_windows:
            close_owned_window(window)
        if s:s.kill()
        if server:server.terminate();server.wait(timeout=5)
    print(root/'receipt.json')
if __name__=='__main__':main()
