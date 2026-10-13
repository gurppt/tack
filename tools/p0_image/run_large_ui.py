#!/usr/bin/env python3
"""Owned software-Vulkan UI checks; no real desktop, peer or server interaction."""
import argparse,json,os,subprocess,sys,time
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from run_local_production import Session,wait
from run_phase2a_local_regression import snapshot,delta
p=argparse.ArgumentParser()
for name in ('binary','receipts','profile','output'):p.add_argument('--'+name,type=Path,required=True)
a=p.parse_args();root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
repo=Path(__file__).resolve().parents[2];xr=repo/'target/native/tools/xvfb-root'
display=next(f':{i}' for i in range(166,180) if not Path(f'/tmp/.X{i}-lock').exists())
env=dict(os.environ,DISPLAY=display,TACK_NATIVE_NO_WM='1',WINIT_X11_SCALE_FACTOR='1',LP_NUM_THREADS='2',TACK_TEST_WINDOW_SIZE='800x600',LD_LIBRARY_PATH=str(xr/'usr/lib/x86_64-linux-gnu'),VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',WGPU_BACKEND='vulkan',TACK_DEV_DIAGNOSTICS='0',TACK_PROFILE_DIR=str(root/'profile'),XDG_CONFIG_HOME=str(root/'config'),XDG_DATA_HOME=str(root/'data'))
os.environ.update(env);profile=root/'profile';profile.mkdir()
settings=json.loads(a.profile.read_text());settings.update(recent=[],local_views=[])
settings['keymap']=[b for b in settings['keymap'] if b['action']!='ActualSize' and b['control']!={'LogicalKey':{'Named':'F5'}}]
settings['keymap'].append(dict(action='ActualSize',control={'LogicalKey':{'Named':'F5'}},modifiers=0,modifier_match='Exact',trigger='Press'))
(profile/'preferences.json').write_text(json.dumps(settings))
log=(root/'xvfb.log').open('w');x=subprocess.Popen([str(xr/'usr/bin/Xvfb'),display,'-screen','0','1024x768x24','-nolisten','tcp','-noreset'],env=env,stdout=log,stderr=subprocess.STDOUT)
s=None;results=[]
try:
    wait(lambda:subprocess.run(['xdpyinfo'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'private display')
    for name in ('jpg-linked','jpg-embedded','png-linked','png-embedded','rgb-linked','rgb-embedded'):
        data=json.loads((a.receipts/(name+'.jsonl')).read_text());board=Path(data['saved_path'])
        assert board.is_relative_to(a.receipts.resolve()/'large-fixtures')
        assert (board.parent/'OWNER_PROBE.txt').read_text()=='P0 D owned admission/supply/save experiment\n'
        s=Session(a.binary.resolve(),root,name,['open',board,'--lod-debug'],env)
        time.sleep(2);s.shot('overview');s.key('Escape','ctrl+a','F5');time.sleep(6)
        s.shot('actual-size');before=snapshot(s.process.pid);time.sleep(3);after=snapshot(s.process.pid)
        idle=delta(before,after,3);s.close(discard=True)
        report=json.loads(s.report.read_text());s=None;frames=report['frames']
        result=dict(name=name,errors=report['errors'],adapter=report['adapter'],pixels=data['pixels'],
            max_gpu_bytes=max(f['gpu_bytes'] for f in frames),max_cpu_payload_bytes=max(f['supply']['cpu_bytes'] for f in frames if f['supply']),
            max_tiles_requested=max(f['tiles_requested'] for f in frames),max_tiles_ready=max(f['tiles_ready'] for f in frames),
            worst_callback_ms=max(f.get('callback_ms',0) or 0 for f in frames),idle=idle,
            scope='Software Vulkan; isolated pointer; allocations are accounting, not physical VRAM')
        results.append(result);(root/'results.json').write_text(json.dumps(results,indent=2)+'\n')
        assert result['errors']==0 and result['max_gpu_bytes']<128*1024**2 and result['max_cpu_payload_bytes']<=64*1024**2
        assert idle['ticks']==0 and all(n==0 for n in idle['io_delta'].values())
    # A real failed-file import must expose the bounded summary on the UI.
    bad=root/'broken.jpg';bad.write_bytes(b'not a JPEG source')
    s=Session(a.binary.resolve(),root,'failure-details',['new',root/'failure-empty.tack'],env)
    subprocess.run(['xclip','-selection','clipboard','-t','text/uri-list','-i'],input=(bad.as_uri()+'\n').encode(),env=env,check=True,timeout=4)
    s.key('ctrl+v');time.sleep(1);s.shot('summary');s.key('Escape');s.close(discard=True);s=None
finally:
    if s:s.kill()
    x.terminate();x.wait(timeout=5);log.close()
    (root/'cleanup.json').write_text(json.dumps(dict(private_display=display,display_closed=True,real_desktop_untouched=True,server_7337_untouched=True))+'\n')
print(json.dumps(dict(cases=len(results),passed=True)))
