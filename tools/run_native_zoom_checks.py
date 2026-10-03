#!/usr/bin/env python3
"""Bounded X11 low/high zoom manipulation check; never saves the input board."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
from PIL import Image
import run_native_image_checks as shared


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--board', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary, board = args.binary.resolve(), args.board.resolve()
    digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    initial_digest = digest(board)
    checks = []
    command = shared.command
    with (root/'native.log').open('w') as log:
        process = subprocess.Popen([str(binary), 'open', str(board), '--seconds', '25', '--output', str(root/'native.json')], stdout=log, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic()+5
            window = None
            while time.monotonic()<deadline:
                try:
                    ids = command('xdotool','search','--onlyvisible','--pid',process.pid).splitlines()
                    if ids:
                        window = ids[-1]
                        break
                except subprocess.CalledProcessError:
                    pass
                time.sleep(.1)
            if window is None:
                raise RuntimeError('owned window did not map')
            command('xdotool','windowactivate','--sync',window)
            command('xdotool','keyup','ctrl','alt','shift','super')
            time.sleep(.3)
            def capture(name):
                time.sleep(.2)
                path = root/f'{name}.png'
                command('import','-window',window,path)
                return Image.open(path).convert('RGB')
            def bounds(im):
                pts = [(x,y) for y in range(im.height) for x in range(im.width) if (lambda c:c[0]>145 and c[1]<130 and c[2]<100)(im.getpixel((x,y)))]
                return min(x for x,y in pts),min(y for x,y in pts),max(x for x,y in pts),max(y for x,y in pts)
            def pointer(p):
                command('xdotool','mousemove','--window',window,round(p[0]),round(p[1]))
                time.sleep(.08)
            def drag(start,end,cancel=False):
                pointer(start)
                command('xdotool','mousedown','1')
                for i in range(1,9):
                    pointer([start[j]+(end[j]-start[j])*i/8 for j in range(2)])
                if cancel:command('xdotool','key','Escape')
                command('xdotool','mouseup','1')
            def record(name,ok):
                checks.append({'name':name,'observed':bool(ok)})
                (root/'checks.json').write_text(json.dumps(checks,indent=2))
                if not ok:raise AssertionError(name)
            start_image = capture('initial')
            l,t,r,b = bounds(start_image)
            center = [(l+r)/2,(t+b)/2]
            pointer(center)
            command('xdotool','click','1')
            time.sleep(.45)
            command('xdotool','click','--repeat','6','--delay','35','5')
            low = capture('low-selected')
            l,t,r,b = bounds(low)
            record('low zoom image remains targetable',20<r-l<100)
            drag((r,b),(r+30,b+20))
            resized = capture('low-resized')
            record('low zoom corner handle resizes',bounds(resized)[2]-bounds(resized)[0]>r-l+15)
            command('xdotool','key','ctrl+z')
            restored = capture('low-restored')
            record('low zoom resize undo restores geometry',bounds(restored)==bounds(low))
            center=[(l+r)/2,(t+b)/2]
            drag(center,(center[0]+45,center[1]+20),cancel=True)
            record('low zoom drag Escape restores geometry',bounds(capture('low-cancelled'))==bounds(low))
            pointer(center)
            command('xdotool','click','--repeat','10','--delay','35','4')
            high=capture('high-selected')
            record('high zoom image fills viewport',bounds(high)[2]-bounds(high)[0]>high.width*.9)
            # Use interior grid pixels when all image edges are outside the viewport.
            center=(high.width/2,high.height/2)
            drag(center,(center[0]+35,center[1]+20))
            moved=capture('high-moved')
            roi=(int(center[0]-100),int(center[1]-80),int(center[0]+100),int(center[1]+80))
            record('high zoom drag visibly tracks pointer',high.crop(roi).tobytes()!=moved.crop(roi).tobytes())
            command('xdotool','key','ctrl+z')
            undone=capture('high-undone')
            record('high zoom undo restores interior pixels',high.crop(roi).tobytes()==undone.crop(roi).tobytes())
            drag(center,(center[0]-40,center[1]+15),cancel=True)
            record('high zoom Escape restores interior pixels',high.crop(roi).tobytes()==capture('high-cancelled').crop(roi).tobytes())
            command('xdotool','key','alt+F4')
            process.wait(timeout=4)
        finally:
            command('xdotool','keyup','ctrl','alt','shift','super')
            if process.poll() is None:
                process.terminate()
                try:process.wait(timeout=2)
                except subprocess.TimeoutExpired:process.kill();process.wait()
    if process.returncode:raise RuntimeError(f'native exit {process.returncode}')
    if digest(board)!=initial_digest:raise AssertionError('input board changed')
    (root/'provenance.json').write_text(json.dumps({'binary_sha256':digest(binary),'board_sha256':initial_digest,'harness_sha256':digest(Path(__file__)),'shared_harness_sha256':digest(Path(shared.__file__)),'returncode':process.returncode,'checks':checks},indent=2))
    print(json.dumps(checks,indent=2))


if __name__=='__main__':main()
