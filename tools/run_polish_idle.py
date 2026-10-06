#!/usr/bin/env python3
"""Phase 1J idle across every theme, grid and application-menu state."""
import argparse
import os,subprocess,json,sys
from pathlib import Path
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--baseline', type=Path, required=True)
parser.add_argument('--board', type=Path, required=True)
parser.add_argument('--profile', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
a = parser.parse_args()
if os.environ.get('DISPLAY') in (None, ':0', ':0.0') or os.environ.get('TACK_NATIVE_NO_WM') != '1':
 parser.error('requires explicitly owned isolated X11 display')
root=a.output.resolve();root.mkdir(parents=True,exist_ok=False);env=dict(os.environ)
board=a.board.resolve();profile=json.loads(a.profile.read_text())
cases=[('baseline',a.baseline.resolve(),None,'none',True),('neutral',a.binary.resolve(),'NeutralGray','none',False),('dark',a.binary.resolve(),'VeryDark','none',False),('light',a.binary.resolve(),'Light','none',False),('menu',a.binary.resolve(),'NeutralGray','application',True),('closed',a.binary.resolve(),'NeutralGray','closed',True)]
for name,binary,theme,menu,baseline in cases:
 profile_dir=root/f'idle-profile-{name}';profile_dir.mkdir()
 if theme:
  current=dict(profile,theme=theme,ui_scale=1,grid=False,recent=[]);(profile_dir/'preferences.json').write_text(json.dumps(current))
 with (root/f'idle-{name}.log').open('w') as log:
  subprocess.run([sys.executable,'tools/run_idle.py','--binary',str(binary),'--board',str(board),'--output',str(root/f'idle-{name}'),'--no-wm','--seconds','5','--context-menu',menu,*(['--baseline'] if baseline else [])],env=dict(env,TACK_PROFILE_DIR=str(profile_dir)),stdout=log,stderr=subprocess.STDOUT,timeout=45,check=True)
 print(name,'idle PASS',flush=True)
