#!/usr/bin/env python3
"""Owned 2A9 X11/Lavapipe window, profile and idle receipts; never uses the desktop."""
import argparse, hashlib, json, os, struct, subprocess, time
from pathlib import Path
from PIL import Image
from run_local_production import Session, wait
from run_native_image_checks import command
from run_phase2a_local_regression import snapshot, delta


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('binary','profile','output'): p.add_argument('--'+name,type=Path,required=True)
    a=p.parse_args();binary=a.binary.resolve();root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
    repo=Path(__file__).resolve().parents[1];xr=repo/'target/native/tools/xvfb-root'
    env=dict(os.environ,DISPLAY=':122',TACK_NATIVE_NO_WM='1',TACK_NATIVE_DIAGNOSTICS='1',WINIT_X11_SCALE_FACTOR='1',LP_NUM_THREADS='2',LD_LIBRARY_PATH=str(xr/'usr/lib/x86_64-linux-gnu'),VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',WGPU_BACKEND='vulkan')
    os.environ.update(env);log=(root/'xvfb.log').open('w')
    x=subprocess.Popen([str(xr/'usr/bin/Xvfb'),':122','-screen','0','1280x1024x24','-nolisten','tcp','-noreset'],env=env,stdout=log,stderr=subprocess.STDOUT)
    s=None;checks=[];idle=[]
    def record(name,ok,detail=None):
        checks.append(dict(name=name,observed=bool(ok),detail=detail));(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if not ok:raise AssertionError(name)
    def move(px,py):command('xdotool','mousemove','--window',s.window,str(px),str(py));time.sleep(.06)
    def click(px,py):move(px,py);command('xdotool','click','1');time.sleep(.15)
    try:
        wait(lambda:subprocess.run(['xdpyinfo'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'owned display')
        im=root/'source.png';Image.new('RGB',(480,320),(225,30,70)).save(im)
        for size in ['800x600','1024x768']:
            r=root/size;r.mkdir();board=r/'board.tack';Image.new('RGB',(480,320),(225,30,70)).save(im)
            subprocess.run([str(binary),'create',str(board),'--linked',str(im)],env=env,check=True,capture_output=True,timeout=20)
            settings=json.loads(a.profile.read_text());settings.update(recent=[],local_views=[],status_bar=True,snapping=False)
            remaps={'F1':'ResetCrop','F2':'ToggleGrayscale','F3':'ToggleLockSelection','F4':'SelectLockedObjects','F5':'ActualSize','F6':'ToggleSourceStorage','F7':'RefreshSource','F8':'EditToolbar','F9':'ExportProfile','F11':'ImportProfile','F12':'CheckForUpdates'}
            remaps.update(PageDown='NextToolbarSet',PageUp='PreviousToolbarSet')
            settings['keymap']=[b for b in settings['keymap'] if b['action'] not in remaps.values() and b['control'] not in [{'LogicalKey':{'Named':k}} for k in remaps if k.startswith('F')]]
            for key,action in remaps.items():settings['keymap'].append(dict(action=action,control={'LogicalKey':{'Named':key}},modifiers=2 if key.startswith('Page') else 0,modifier_match='Exact',trigger='Press'))
            actions=settings['toolbar']['actions'];settings['toolbar_sets']=[dict(name='IMAGE',actions=actions),dict(name='DRAW',actions=['SelectTool:Text','Separator','Undo'])];settings['active_toolbar_set']=0
            # Use a stable action ID from the existing keymap rather than the label.
            text_id=next(action for action in actions if action.startswith('SelectTool') and 'Text' in action)
            settings['toolbar_sets'][1]['actions'][0]=text_id
            profile=r/'profile';profile.mkdir();prefs=profile/'preferences.json';prefs.write_text(json.dumps(settings))
            env2=dict(env,TACK_PROFILE_DIR=str(profile),TACK_TEST_WINDOW_SIZE=size)
            s=Session(binary,r,'ux',['open',board],env2);time.sleep(.6);s.key('Escape','ctrl+a');base=s.save(board)
            s.key('F2');gray=s.save(board);record(size+' grayscale persisted',gray['objects'][0].get('grayscale') is True)
            s.key('ctrl+z');record(size+' grayscale exact Undo',s.save(board)==base);s.key('ctrl+y');record(size+' grayscale Redo',s.save(board)==gray)
            s.key('F3');locked=s.save(board);record(size+' object lock persisted',locked['objects'][0].get('locked') is True)
            s.key('Escape','ctrl+a','F4','F3');unlocked=s.save(board);record(size+' locked selection recovery',not unlocked['objects'][0].get('locked',False))
            s.key('F5');actual=s.save(board);record(size+' Actual Size uses intrinsic dimensions',actual['objects'][0]['size']==(480.,320.))
            s.key('F6');embedded=s.save(board);info=json.loads(subprocess.check_output([str(binary),'inspect',str(board)]));record(size+' Linked to Embedded',info['embedded']==1)
            s.key('F6');s.save(board);info=json.loads(subprocess.check_output([str(binary),'inspect',str(board)]));record(size+' Embedded to remembered Linked',info['embedded']==0)
            Image.new('RGB',(480,320),(20,230,60)).save(im);s.key('F7');time.sleep(.8);s.shot('refreshed');pixel=Image.open(r/'ux-refreshed.png').convert('RGB').getpixel(tuple(int(v)//2 for v in size.split('x')));record(size+' refreshed pixels replace old supply',min(pixel)>180 and max(pixel)-min(pixel)<=1,pixel);record(size+' Refresh preserves geometry',s.save(board)['objects'][0]==actual['objects'][0])
            s.key('ctrl+Next');wait(lambda:json.loads(prefs.read_text())['active_toolbar_set']==1,'next set persisted')
            record(size+' next set content',json.loads(prefs.read_text())['toolbar']['actions']==settings['toolbar_sets'][1]['actions'])
            s.shot('next-set');s.key('ctrl+Prior');wait(lambda:json.loads(prefs.read_text())['active_toolbar_set']==0,'previous set persisted')
            s.key('F8');s.shot('toolbar-sets');s.key('Escape')
            exported=r/'studio.tackprofile';s.picker('F9',exported,selects_file=False);wait(exported.is_file,'profile export')
            portable=json.loads(exported.read_text());record(size+' portable sets and path stripping',len(portable['preferences']['toolbar_sets'])==2 and not portable['preferences']['recent'] and portable['preferences']['last_board_directory'] is None)
            s.key('ctrl+Next');s.picker('F11',exported,selects_file=True);wait(lambda:json.loads(prefs.read_text())['active_toolbar_set']==0,'import replaces active set')
            record(size+' backup before profile import',(profile/'before-import.tackprofile').is_file());s.key('Escape')
            s.key('F10');s.shot('menu');s.key('Escape');s.key('ctrl+comma');s.shot('preferences');s.key('Escape')
            s.key('F12');s.shot('update');time.sleep(.3);record(size+' Update remains open',s.process.poll() is None);s.key('Escape')
            move(10,10);time.sleep(2.);before=snapshot(s.process.pid);time.sleep(2.);after=snapshot(s.process.pid);idle.append(delta(before,after,2.))
            record(size+' idle has no recurring CPU activity',idle[-1]['ticks']==0,idle[-1]);s.close();s=None
        (root/'receipt.json').write_text(json.dumps(dict(passed=True,checks=len(checks),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),build_info=json.loads(subprocess.check_output([str(binary),'--build-info'])),idle=idle,scope='Owned Xvfb; software Vulkan, two rendering threads; physical Windows/hardware UX remains human acceptance.'),indent=2)+'\n')
    finally:
        if s:s.kill()
        x.terminate();x.wait(timeout=5);log.close()


if __name__=='__main__':main()
