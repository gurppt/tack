#!/usr/bin/env python3
"""Real X11 mouse/key/save checks, generated owned images only, bounded lifetime."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import time
from PIL import Image, ImageDraw
from run_native_image_checks import command


def saved_objects(path):
    """Read only harness-owned valid snapshots to assert native committed geometry."""
    data = path.read_bytes(); schema = struct.unpack_from('<I', data, 12)[0]
    sources, assets, objects = struct.unpack_from('<III', data, 48)
    p = 96
    for _ in range(sources):
        size = struct.unpack_from('<I', data, p)[0]; p += 4 + size
    p += assets * 42; records = []
    for _ in range(objects):
        version, kind = struct.unpack_from('<HH', data, p); p += 4
        identity = int.from_bytes(data[p:p+16], 'little'); p += 16
        if version != 1: raise ValueError('fixture record version')
        if kind == 1:
            p += 16
            center = struct.unpack_from('<dd', data, p); size = struct.unpack_from('<dd', data, p+16)
            rotation = struct.unpack_from('<d', data, p+32)[0]; p += 83
            records.append({'id': identity, 'kind': 'image', 'center': center, 'size': size, 'rotation': rotation})
        elif kind == 2:
            center = struct.unpack_from('<dd', data, p); size = struct.unpack_from('<dd', data, p+16); p += 32
            length = struct.unpack_from('<H', data, p)[0]; p += 2
            name = data[p:p+length].decode(); p += length
            records.append({'id': identity, 'kind': 'frame', 'center': center, 'size': size, 'name': name})
        else: raise ValueError('fixture kind')
    p += objects * 16
    groups = struct.unpack_from('<I', data, p)[0] if schema == 2 else 0
    return {'schema': schema, 'objects': records, 'groups': groups}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=Path('target/release/tack-app'))
    parser.add_argument('--missing-link', action='store_true')
    args = parser.parse_args(); root = args.output.resolve(); root.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    for name, color in [('a', (210,75,40)), ('b', (40,130,205))]:
        image = Image.new('RGB', (320,240), color); draw = ImageDraw.Draw(image)
        for x in range(0,320,40): draw.line((x,0,x,239), fill='black', width=2)
        for y in range(0,240,40): draw.line((0,y,319,y), fill='black', width=2)
        image.save(root / f'{name}.png')
    board = root / 'board.tack'
    subprocess.run([str(binary),'create',str(board),'--linked',str(root/'a.png'),str(root/'b.png')],stdout=(root/'create.log').open('w'),stderr=subprocess.STDOUT,check=True,timeout=20)
    hidden = root/'a.png.hidden'
    if args.missing_link: (root/'a.png').rename(hidden)
    checks = []; started = time.monotonic()
    with (root/'native.log').open('w') as log:
        process = subprocess.Popen([str(binary),'open',str(board),'--output',str(root/'native.json')],stdout=log,stderr=subprocess.STDOUT)
        try:
            window = None
            while time.monotonic()-started < 6:
                try:
                    found=command('xdotool','search','--onlyvisible','--pid',process.pid).splitlines()
                    if found: window=found[-1]; break
                except subprocess.CalledProcessError: pass
                time.sleep(.05)
            if window is None: raise RuntimeError('owned window not mapped')
            command('xdotool','windowactivate','--sync',window); time.sleep(.5)
            command('xdotool','keyup','ctrl','alt','shift','super','x')
            def key(sequence): command('xdotool','key',sequence); time.sleep(.13)
            def pointer(x,y): command('xdotool','mousemove','--window',window,round(x),round(y)); time.sleep(.06)
            def capture(name):
                time.sleep(.18);path=root/f'{name}.png'
                for _ in range(3):
                    shot=subprocess.run(['import','-window',str(window),str(path)],capture_output=True,text=True,timeout=4)
                    if shot.returncode==0:return Image.open(path).convert('RGB')
                    time.sleep(.1)
                raise RuntimeError(shot.stderr)
            def title(): return command('xdotool','getwindowname',window)
            def record(name,ok):
                checks.append({'name':name,'observed':bool(ok)});(root/'checks.json').write_text(json.dumps(checks,indent=2))
                if not ok: raise AssertionError(name)
                if time.monotonic()-started>55: raise RuntimeError('native check deadline')
            def save():
                key('ctrl+s')
                for _ in range(50):
                    if 'saved' in title() and 'saving' not in title(): break
                    time.sleep(.04)
                else: raise AssertionError('save deadline')
                return saved_objects(board)
            def drag(start,end,cancel=False):
                pointer(*start);command('xdotool','mousedown','1')
                for i in range(1,6): pointer(start[0]+(end[0]-start[0])*i/5,start[1]+(end[1]-start[1])*i/5)
                if cancel: key('Escape')
                command('xdotool','mouseup','1');time.sleep(.13)
            initial=capture('initial'); center=(640,360)
            if args.missing_link: record('missing source retains prepared image','1 missing' in title())
            key('g');grid=capture('grid-visible');record('grid toggle adds dots without changing history','saved' in title() and initial.tobytes()!=grid.tobytes())
            pointer(*center);command('xdotool','click','5');low=capture('grid-low');command('xdotool','click','4');command('xdotool','click','4');high=capture('grid-high');record('grid remains visible across zoom',low.tobytes()!=high.tobytes());command('xdotool','click','5')
            pointer(*center);command('xdotool','click','1');key('shift+g')
            command('xdotool','mousedown','1');pointer(center[0]+36,center[1]); snapped=capture('snap-active')
            command('xdotool','keydown','x');time.sleep(.12);bypassed=capture('snap-bypassed');record('X changes active snapped preview',snapped.tobytes()!=bypassed.tobytes())
            command('xdotool','keyup','x');time.sleep(.12);restored=capture('snap-restored');record('release restores same stable snap',restored.tobytes()==snapped.tobytes())
            key('Escape');command('xdotool','mouseup','1');record('Escape has no committed snap edit','saved' in title());key('shift+g')
            key('ctrl+a');key('ctrl+g');grouped=save();record('group saves membership',grouped['groups']==1)
            pointer(10,10);command('xdotool','click','1');pointer(*center);command('xdotool','click','1');time.sleep(.12);record('member selects entire group','2 selected' in title())
            drag(center,(center[0]+40,center[1]+20));moved=save();record('group members move by equal delta',all(abs((m['center'][axis]-g['center'][axis])-(moved['objects'][0]['center'][axis]-grouped['objects'][0]['center'][axis]))<1e-8 for m,g in zip(moved['objects'],grouped['objects']) for axis in (0,1)) and moved['objects']!=grouped['objects'])
            key('ctrl+z');record('group gesture undo exact',save()==grouped)
            key('ctrl+shift+g');ungrouped=save();record('ungroup preserves transforms',ungrouped['groups']==0 and ungrouped['objects']==grouped['objects'])
            image=capture('before-align');blue=[(i%image.width,i//image.width) for i,c in enumerate(image.getdata()) if c[2]>150 and c[1]>80 and c[0]<100]
            blue_center=((min(p[0] for p in blue)+max(p[0] for p in blue))/2,(min(p[1] for p in blue)+max(p[1] for p in blue))/2)
            pointer(10,10);command('xdotool','click','1');pointer(*blue_center);command('xdotool','click','1');time.sleep(.13)
            record('alignment fixture selects one ungrouped member','1 selected' in title())
            drag(blue_center,(blue_center[0],blue_center[1]+40));displaced=save()
            record('alignment fixture has distinct world top edges',len({round(o['center'][1]-o['size'][1]/2,8) for o in displaced['objects']})==2)
            key('ctrl+a');key('ctrl+Up');aligned=save();record('top alignment shares world edge',len({round(o['center'][1]-o['size'][1]/2,8) for o in aligned['objects']})==1 and aligned['objects']!=displaced['objects']);key('ctrl+z');record('alignment undo exact',save()==displaced)
            key('ctrl+shift+v');distributed=save();record('vertical distribution changes layout',distributed['objects']!=displaced['objects']);key('ctrl+z');record('distribution undo exact',save()==displaced);key('ctrl+z')
            key('ctrl+shift+f');created=save();record('create named frame uses schema 2',created['schema']==2 and len(created['objects'])==3)
            key('F2');command('xdotool','type','--clearmodifiers','--delay','12','Zone Alpha');key('Return');named=save();record('F2 name saved exact',named['objects'][-1].get('name')=='Zone Alpha')
            key('space');focused=capture('frame-focused');record('frame focus shows label',focused.tobytes()!=initial.tobytes())
            # Selected frame bottom/right handle is visible after focus. Image pixels stay unchanged.
            frame=named['objects'][-1];view_scale=min(1280/frame['size'][0],720/frame['size'][1])*.9
            edge=(640+frame['size'][0]*view_scale/2,360)
            drag(edge,(edge[0]+30,edge[1]));resized=save();record('frame edge resize preserves images',resized['objects'][:2]==named['objects'][:2] and resized['objects'][-1]['size'][0]>frame['size'][0]);key('ctrl+z')
            # A top border away from handles selects/drags only the frame.
            border=(640+40,360-frame['size'][1]*view_scale/2)
            drag(border,(border[0]+40,border[1]+30));frame_moved=save();record('frame move leaves contained images fixed',frame_moved['objects'][:2]==named['objects'][:2] and frame_moved['objects'][-1]['center']!=frame['center']);key('ctrl+z')
            key('Next');key('Prior');record('frame navigation preserves selection','1 selected' in title())
            key('Delete');deleted=save();record('delete frame preserves images',len(deleted['objects'])==2 and deleted['objects']==named['objects'][:2]);key('ctrl+z');record('undo frame delete exact',save()==named)
            final=capture('final');key('alt+F4');process.wait(timeout=5)
        finally:
            if hidden.exists(): hidden.rename(root/'a.png')
            command('xdotool','keyup','ctrl','alt','shift','super','x')
            if process.poll() is None:
                process.terminate()
                try: process.wait(timeout=2)
                except subprocess.TimeoutExpired: process.kill();process.wait()
    if process.returncode: raise RuntimeError(f'native exit {process.returncode}')
    (root/'summary.json').write_text(json.dumps({'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'harness_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'missing_link':args.missing_link,'scale':os.environ.get('WINIT_X11_SCALE_FACTOR','desktop'),'checks':checks,'returncode':process.returncode},indent=2)+'\n')
    print(json.dumps(checks,indent=2))


if __name__=='__main__': main()
