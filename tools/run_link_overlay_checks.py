#!/usr/bin/env python3
"""Re-run the delivered Link overflow fixture on an owned software-GPU display."""
import argparse, hashlib, json, os, subprocess, time
from pathlib import Path
from PIL import Image
from run_local_production import Session, wait
from run_native_image_checks import command


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'profile', 'output'): p.add_argument('--'+name, type=Path, required=True)
    a=p.parse_args(); binary=a.binary.resolve(); root=a.output.resolve(); root.mkdir(parents=True,exist_ok=False)
    repo=Path(__file__).resolve().parents[1]; xr=repo/'target/native/tools/xvfb-root'
    env=dict(os.environ,DISPLAY=':121',TACK_NATIVE_NO_WM='1',TACK_NATIVE_DIAGNOSTICS='1',WINIT_X11_SCALE_FACTOR='1',LP_NUM_THREADS='2',TACK_TEST_WINDOW_SIZE='1280x1024',LD_LIBRARY_PATH=str(xr/'usr/lib/x86_64-linux-gnu'),VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',WGPU_BACKEND='vulkan')
    os.environ.update(env); log=(root/'xvfb.log').open('w')
    x=subprocess.Popen([str(xr/'usr/bin/Xvfb'),':121','-screen','0','1280x1024x24','-nolisten','tcp','-noreset'],env=env,stdout=log,stderr=subprocess.STDOUT)
    s=None; checks=[]
    def record(name,ok,detail=None):
        checks.append(dict(name=name,observed=bool(ok),detail=detail));(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if not ok:raise AssertionError(name)
    def move(px,py):command('xdotool','mousemove','--window',s.window,str(px),str(py));time.sleep(.05)
    try:
        wait(lambda:subprocess.run(['xdpyinfo'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'owned display')
        im=root/'fixture.png';Image.new('RGB',(160,100),(40,120,190)).save(im)
        board=root/'board.tack';subprocess.run([str(binary),'create',str(board),'--embedded',*[str(im)]*32],env=env,check=True,capture_output=True,timeout=20)
        settings=json.loads(a.profile.read_text());settings.update(recent=[],local_views=[],status_bar=True)
        remaps={'F1':'LinkToFrame','F6':'ToggleAnnotationSelectionLock','F7':'CreateFrame'}
        settings['keymap']=[b for b in settings['keymap'] if b['action'] not in remaps.values() and b['control'] not in [{'LogicalKey':{'Named':key}} for key in remaps]]
        for key,action in remaps.items():settings['keymap'].append(dict(action=action,control={'LogicalKey':{'Named':key}},modifiers=0,modifier_match='Exact',trigger='Press'))
        profile=root/'profile';profile.mkdir();(profile/'preferences.json').write_text(json.dumps(settings))
        s=Session(binary,root,'link',['open',board],dict(env,TACK_PROFILE_DIR=str(profile)));time.sleep(.7);s.key('Escape');move(640,512)
        command('xdotool','click','--repeat','15','--delay','40','5');time.sleep(.2)
        s.key('ctrl+a','F7','F6','ctrl+a');base=s.save(board);move(100,100);s.key('F1');time.sleep(.4)
        record('32-unit delivered-crash fixture stays alive',s.process.poll() is None,s.title());s.shot('large-dots')
        start=time.monotonic()-s.started
        for k in range(32):move(100+k*3,100+k%7*9)
        end=time.monotonic()-s.started
        s.key('Escape');time.sleep(.3);s.key('F1');move(700,512);command('xdotool','click','1');time.sleep(.4)
        linked=s.save(board);record('all 32 Link operations complete despite bounded preview',len(linked.get('links',[]))==32,linked.get('links',[]))
        s.key('ctrl+z');record('Link Undo exact',s.save(board)==base);s.key('ctrl+y');record('Link Redo exact',s.save(board)==linked)
        s.key('F1');move(100,100);s.shot('final-preview');s.key('Escape');time.sleep(.8)
        idle_start=time.monotonic()-s.started;time.sleep(2.);idle_end=time.monotonic()-s.started
        s.close();report=json.loads(s.report.read_text());s=None
        stats=max((frame['link_preview'] for frame in report['frames']),key=lambda value:value['lines'])
        record('one logical quad per unit, bounded vertices',stats['units']==32 and stats['lines']==32 and stats['vertices']==192,stats)
        idle_frames=[frame for frame in report['frames'] if idle_start<=frame['elapsed_ms']/1000<=idle_end]
        record('Escape clears Link preview and no recurring redraw follows',report['link_preview']['lines']==0 and not idle_frames,dict(final=report['link_preview'],idle_seconds=2,redraws=len(idle_frames)))
        samples=[v for v in report['frames'] if start<=v['elapsed_ms']/1000<=end]
        (root/'receipt.json').write_text(json.dumps(dict(checks=len(checks),passed=all(c['observed'] for c in checks),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),build_info=json.loads(subprocess.check_output([str(binary),'--build-info'])),link_preview=stats,old_primitives=4096,old_limit=2048,new_logical_quads=32,new_vertices=192,overlay_passes_per_frame=1,shader_pattern='4 logical pixels on / 4 off, no per-dash vertex geometry',frames=samples,report=report),indent=2)+'\n')
    finally:
        if s:s.kill()
        x.terminate();x.wait(timeout=5);log.close()
if __name__=='__main__':main()
