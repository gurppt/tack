#!/usr/bin/env python3
"""Owned native regression: persisted LOD revision reuse and focus revalidation."""
import argparse, hashlib, json, os, subprocess, time
from pathlib import Path
from PIL import Image
from run_local_production import Session, wait
from run_native_image_checks import command
from run_phase2a_local_regression import snapshot, delta

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for n in ('binary','profile','output'): p.add_argument('--'+n,type=Path,required=True)
    a=p.parse_args();binary=a.binary.resolve();root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
    xr=Path(__file__).resolve().parents[1]/'target/native/tools/xvfb-root'
    env=dict(os.environ,DISPLAY=':124',TACK_NATIVE_NO_WM='1',TACK_NATIVE_DIAGNOSTICS='1',TACK_TEST_WINDOW_SIZE='800x600',WINIT_X11_SCALE_FACTOR='1',LP_NUM_THREADS='2',LD_LIBRARY_PATH=str(xr/'usr/lib/x86_64-linux-gnu'),VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',WGPU_BACKEND='vulkan')
    os.environ.update(env);log=(root/'xvfb.log').open('w');s=None;outside=None;checks=[]
    x=subprocess.Popen([str(xr/'usr/bin/Xvfb'),':124','-screen','0','1024x768x24','-nolisten','tcp','-noreset'],env=env,stdout=log,stderr=subprocess.STDOUT)
    def record(name,ok,detail=None):
        checks.append(dict(name=name,observed=bool(ok),detail=detail));(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if not ok: raise AssertionError(name)
    def pixels(name,color):
        def matches():
            s.shot(name);v=Image.open(root/f'{s.name}-{name}.png').convert('RGB').getpixel((400,300))
            return v if v[color]>160 and all(v[i]<75 for i in range(3) if i!=color) else None
        v=wait(matches,name,12);record(name,True,v)
    try:
        wait(lambda:subprocess.run(['xdpyinfo'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'display')
        profile=root/'profile';profile.mkdir();settings=json.loads(a.profile.read_text());settings.update(recent=[],local_views=[])
        mapping={'F6':'ToggleSourceStorage','F7':'RefreshSource'}
        settings['keymap']=[b for b in settings['keymap'] if b['action'] not in mapping.values() and b['control'] not in [{'LogicalKey':{'Named':k}} for k in mapping]]
        for k,v in mapping.items(): settings['keymap'].append(dict(action=v,control={'LogicalKey':{'Named':k}},modifiers=0,modifier_match='Exact',trigger='Press'))
        (profile/'preferences.json').write_text(json.dumps(settings));env['TACK_PROFILE_DIR']=str(profile)
        source=root/'original.jpg';board=root/'board.tack';Image.new('RGB',(1920,1080),(230,20,20)).save(source,quality=92)
        subprocess.run([str(binary),'create',str(board),'--linked',str(source)],env=env,check=True,capture_output=True,timeout=20)
        s=Session(binary,root,'warm',['open',board],env);s.key('Escape','ctrl+a');baseline=s.save(board)
        pixels('initial-red',0);s.key('F7');s.save(board);pixels('refresh-red-revision-2',0)
        s.key('F6');s.save(board);pixels('embedded-red-revision-3',0);s.key('F6');s.save(board);pixels('linked-red-revision-4',0)
        s.key('ctrl+z','ctrl+z','ctrl+z');record('undo restores original source geometry',s.save(board)==baseline);s.close();s=None
        Image.new('RGB',(1920,1080),(20,230,20)).save(source,quality=92)
        s=Session(binary,root,'changed',['open',board],env);s.key('Escape','ctrl+a');pixels('open-refreshes-green-without-command',1)
        record('automatic refresh preserves object geometry',s.save(board)==baseline)
        s.key('F6');s.save(board);pixels('embedded-green-despite-old-revision-3-cache',1)
        s.key('F6');s.save(board);pixels('linked-green-despite-old-revision-4-cache',1)
        outside=subprocess.Popen(['xmessage','-title','TackLinkProbeOutside','-name','TackLinkProbeOutside','outside editing'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        def outside_window():
            result=subprocess.run(['xdotool','search','--onlyvisible','--name','^TackLinkProbeOutside$'],capture_output=True,text=True,timeout=3)
            return result.stdout.splitlines()[-1] if result.stdout.strip() else None
        window=wait(outside_window,'outside window')
        command('xdotool','windowfocus','--sync',window);Image.new('RGB',(1920,1080),(20,20,230)).save(source,quality=92)
        s.focus();pixels('focus-refreshes-blue-without-command',2);record('focus refresh preserves geometry',s.save(board)==baseline)
        command('xdotool','mousemove','--window',s.window,'10','10');time.sleep(2);before=snapshot(s.process.pid);time.sleep(2);idle=delta(before,snapshot(s.process.pid),2)
        record('settled probe does not poll or redraw',idle['ticks']==0 and idle['io_delta']['read_bytes']==0 and idle['io_delta']['write_bytes']==0,idle)
        s.close();s=None
        (root/'receipt.json').write_text(json.dumps(dict(passed=True,checks=len(checks),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),build_info=json.loads(subprocess.check_output([str(binary),'--build-info'])),scope='Owned Xvfb/software Vulkan; persistent cache warmed before revision reuse. No user windows/files.'),indent=2)+'\n')
    finally:
        if s:s.kill()
        if outside:outside.terminate();outside.wait(timeout=5)
        x.terminate();x.wait(timeout=5);log.close()
if __name__=='__main__':main()
