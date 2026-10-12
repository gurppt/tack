#!/usr/bin/env python3
"""Disposable Wine COM smoke tests, on private X only; not physical Win10 proof."""
import argparse,json,os,shutil,subprocess,tempfile,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--probe',type=Path,required=True);p.add_argument('--tests',type=Path);p.add_argument('--test-filter');p.add_argument('--output',type=Path,required=True);p.add_argument('--modes',nargs='+',default=['pre-cancel','invalid-owner','cancel-open','cancel-save','cancel-folder','cancel-multiple','cancel-open','repeat-cancel']);a=p.parse_args()
root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
if shutil.disk_usage('/dev/shm').free<3*1024**3:raise RuntimeError('3 GiB private tmpfs reserve required')
prefix=Path(tempfile.mkdtemp(prefix='tack-p0-dialog-owned-',dir='/dev/shm'));(prefix/'OWNER.txt').write_text('P0-WIN-IMAGE-CORE-02-B owned Wine prefix\n')
repo=Path(__file__).resolve().parents[2];xr=repo/'target/native/tools/xvfb-root';display=next(f':{i}' for i in range(166,180) if not Path(f'/tmp/.X{i}-lock').exists())
env=dict(os.environ,DISPLAY=display,WINEPREFIX=str(prefix),WINEARCH='win64',WINEDEBUG='-all',WINEDLLOVERRIDES='mscoree,mshtml=d',LIBGL_ALWAYS_SOFTWARE='1',LD_LIBRARY_PATH=str(xr/'usr/lib/x86_64-linux-gnu'))
xlog=(root/'xvfb.log').open('w');x=subprocess.Popen([str(xr/'usr/bin/Xvfb'),display,'-screen','0','1024x768x24','-nolisten','tcp','-noreset'],env=env,stdout=xlog,stderr=subprocess.STDOUT);receipts=[]
try:
    deadline=time.monotonic()+10
    while subprocess.run(['xdpyinfo'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode:
        if time.monotonic()>deadline:raise RuntimeError('private X display unavailable')
        time.sleep(.1)
    for mode in a.modes:
        start=time.monotonic();r=subprocess.run(['/usr/bin/wine',str(a.probe.resolve()),mode],env=env,capture_output=True,text=True,timeout=40)
        receipts.append(dict(mode=mode,code=r.returncode,elapsed_s=time.monotonic()-start,stdout=r.stdout,stderr=r.stderr[-4000:],scope='Wine on private Xvfb, not physical Windows'))
        (root/'results.json').write_text(json.dumps(receipts,indent=2)+'\n')
        if r.returncode:raise RuntimeError(f'native probe failed: {mode}')
    if a.tests:
        r=subprocess.run(['/usr/bin/wine',str(a.tests.resolve()),*([a.test_filter] if a.test_filter else [])],env=env,capture_output=True,text=True,timeout=40)
        (root/'native-unit-tests.txt').write_text(r.stdout+r.stderr)
        if r.returncode:raise RuntimeError('native unit tests failed')
finally:
    subprocess.run(['/usr/bin/wineserver','-k'],env=env,timeout=10,check=False)
    subprocess.run(['/usr/bin/wineserver','-w'],env=env,timeout=10,check=False)
    x.terminate();x.wait(timeout=5);xlog.close()
    assert (prefix/'OWNER.txt').read_text()=='P0-WIN-IMAGE-CORE-02-B owned Wine prefix\n'
    shutil.rmtree(prefix)
    (root/'cleanup.json').write_text(json.dumps(dict(owned_prefix_removed=True,private_display=display,production_server_untouched=True))+'\n')
print(json.dumps(dict(checks=len(receipts),passed=all(r['code']==0 for r in receipts))))
