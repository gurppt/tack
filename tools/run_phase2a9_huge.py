#!/usr/bin/env python3
"""Owned single-worker native Actual Size check against an existing huge JPEG."""
import argparse, hashlib, json, os, subprocess, time
from pathlib import Path
from run_local_production import Session, wait
from run_phase2a_local_regression import snapshot, delta

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for n in ('binary','source','profile','output'):p.add_argument('--'+n,type=Path,required=True)
    a=p.parse_args();root=a.output.resolve();root.mkdir(parents=True,exist_ok=False);binary=a.binary.resolve()
    repo=Path(__file__).resolve().parents[1];xr=repo/'target/native/tools/xvfb-root'
    env=dict(os.environ,DISPLAY=':123',TACK_NATIVE_NO_WM='1',TACK_NATIVE_DIAGNOSTICS='1',WINIT_X11_SCALE_FACTOR='1',LP_NUM_THREADS='2',TACK_TEST_WINDOW_SIZE='800x600',LD_LIBRARY_PATH=str(xr/'usr/lib/x86_64-linux-gnu'),VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',WGPU_BACKEND='vulkan')
    os.environ.update(env);log=(root/'xvfb.log').open('w');s=None
    x=subprocess.Popen([str(xr/'usr/bin/Xvfb'),':123','-screen','0','1024x768x24','-nolisten','tcp','-noreset'],env=env,stdout=log,stderr=subprocess.STDOUT)
    try:
        wait(lambda:subprocess.run(['xdpyinfo'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'owned display')
        board=root/'huge.tack';subprocess.run([str(binary),'create',str(board),'--linked',str(a.source.resolve())],env=env,check=True,capture_output=True,timeout=60)
        profile=root/'profile';profile.mkdir();settings=json.loads(a.profile.read_text());settings.update(recent=[],local_views=[])
        settings['keymap']=[b for b in settings['keymap'] if b['action']!='ActualSize' and b['control']!={'LogicalKey':{'Named':'F5'}}]
        settings['keymap'].append(dict(action='ActualSize',control={'LogicalKey':{'Named':'F5'}},modifiers=0,modifier_match='Exact',trigger='Press'))
        (profile/'preferences.json').write_text(json.dumps(settings))
        s=Session(binary,root,'huge',['open',board,'--potato'],dict(env,TACK_PROFILE_DIR=str(profile)))
        time.sleep(2.);s.key('Escape','ctrl+a','F5');saved=s.save(board)
        assert saved['objects'][0]['size']==(50000.,50000.)
        time.sleep(6.);s.shot('actual-size');before=snapshot(s.process.pid);time.sleep(3.);after=snapshot(s.process.pid)
        idle=delta(before,after,3.);s.close();report=json.loads(s.report.read_text());s=None
        frames=report['frames'];max_gpu=max(f['gpu_bytes'] for f in frames);max_cpu=max(f['supply']['cpu_bytes'] for f in frames if f['supply'])
        assert max_gpu<128*1024*1024 and max_cpu<64*1024*1024
        assert idle['ticks']==0
        (root/'receipt.json').write_text(json.dumps(dict(passed=True,binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),build_info=json.loads(subprocess.check_output([str(binary),'--build-info'])),intrinsic=[50000,50000],full_rgba_bytes=10000000000,max_gpu_bytes=max_gpu,max_cpu_payload_bytes=max_cpu,max_tiles_requested=max(f['tiles_requested'] for f in frames),max_tiles_ready=max(f['tiles_ready'] for f in frames),frames=len(frames),idle=idle,scope='Software Vulkan, one supply worker; CPU payload and reported GPU residence exclude native decoder scratch and OS file cache.'),indent=2)+'\n')
    finally:
        if s:s.kill()
        x.terminate();x.wait(timeout=5);log.close()

if __name__=='__main__':main()
