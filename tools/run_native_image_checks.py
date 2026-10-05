#!/usr/bin/env python3
"""Observed native X11 checks on generated owned fixtures; never use private boards."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
from PIL import Image, ImageDraw


def command(*args):
    # Opt-in isolated Xvfb protocol: no desktop WM or global close shortcut.
    args=list(map(str,args))
    if os.environ.get('TACK_NATIVE_NO_WM')=='1' and args[:2]==['xdotool','windowactivate']:
        args[1]='windowfocus'
    if os.environ.get('TACK_NATIVE_NO_WM')=='1' and args==['xdotool','key','alt+F4']:
        from run_idle import close_owned_window
        window=subprocess.check_output(['xdotool','getwindowfocus'],text=True,timeout=4).strip()
        close_owned_window(window)
        time.sleep(.2)
        result=subprocess.run(['xdotool','getwindowname',window],text=True,capture_output=True,timeout=4)
        if result.returncode==0 and 'unsaved' in result.stdout:
            subprocess.run(['xdotool','key','Down','Return'],check=True,timeout=4)
        return ''
    return subprocess.check_output(args, text=True, timeout=4).strip()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--binary',type=Path,default=Path('target/release/tack-app'))
    parser.add_argument('--missing-link',action='store_true',help='Hide only the generated a.png, retaining its stored preview')
    args=parser.parse_args()
    root=args.output.resolve();root.mkdir(parents=True,exist_ok=False)
    binary=args.binary.resolve()
    scale=max(1, int(float(os.environ.get('WINIT_X11_SCALE_FACTOR','1')) + .5))
    for name,color in [('a',(210,75,40)),('b',(40,130,205))]:
        image=Image.new('RGB',(320,240),color);draw=ImageDraw.Draw(image)
        for x in range(0,320,40):draw.line((x,0,x,239),fill='black',width=2)
        for y in range(0,240,40):draw.line((0,y,319,y),fill='black',width=2)
        draw.rectangle((3,3,29,29),fill='white');image.save(root/f'{name}.png')
    board=root/'board.tack'
    with (root/'create.log').open('w') as log:
        subprocess.run([str(binary),'create',str(board),'--linked',str(root/'a.png'),str(root/'b.png')],stdout=log,stderr=subprocess.STDOUT,check=True,timeout=20)
    hidden=root/'a.png.hidden'
    if args.missing_link:(root/'a.png').rename(hidden)
    (root/'provenance.json').write_text(json.dumps({'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'harness_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'requested_scale':scale,'missing_link':args.missing_link},indent=2))
    checks=[]
    with (root/'native.log').open('w') as log:
        process=subprocess.Popen([str(binary),'open',str(board),'--seconds','45','--output',str(root/'native.json')],stdout=log,stderr=subprocess.STDOUT)
        try:
            started=time.monotonic();window=None
            while time.monotonic()-started<5:
                try:
                    ids=command('xdotool','search','--onlyvisible','--pid',process.pid).splitlines()
                    if ids:window=ids[-1];break
                except subprocess.CalledProcessError:pass
                time.sleep(.1)
            if not window:raise RuntimeError('owned window did not map')
            command('xdotool','windowactivate','--sync',window);time.sleep(.3)
            command('xdotool','keyup','ctrl','alt','shift','super')
            def capture(name):
                time.sleep(.18);path=root/f'{name}.png';command('import','-window',window,path);return path
            def bounds(path):
                image=Image.open(path).convert('RGB');points=[(x,y) for y in range(image.height) for x in range(image.width) if (lambda c:c[0]>145 and c[1]<130 and c[2]<100)(image.getpixel((x,y)))]
                if not points:raise RuntimeError('fixture not visible')
                return min(x for x,y in points),min(y for x,y in points),max(x for x,y in points),max(y for x,y in points)
            def pointer(x,y):command('xdotool','mousemove','--window',window,round(x),round(y));time.sleep(.08)
            def title():return command('xdotool','getwindowname',window)
            def record(name,passed):
                row={'name':name,'observed':bool(passed),'title':title()};checks.append(row);(root/'checks.json').write_text(json.dumps(checks,indent=2))
                if not passed:raise AssertionError(row)
            def drag(start,end,mods=None,cancel=False):
                pointer(*start)
                if mods:command('xdotool','keydown',mods)
                command('xdotool','mousedown','1');time.sleep(.1)
                for i in range(1,9):pointer(start[0]+(end[0]-start[0])*i/8,start[1]+(end[1]-start[1])*i/8)
                if cancel:command('xdotool','key','Escape')
                command('xdotool','mouseup','1')
                if mods:command('xdotool','keyup',mods)
                time.sleep(.1)
            before=capture('initial');left,top,right,bottom=bounds(before);center=((left+right)/2,(top+bottom)/2)
            if args.missing_link:record('missing linked preview remains visible','1 missing' in title())
            initial_image=Image.open(before).convert('RGB')
            blue=[(x,y) for y in range(initial_image.height) for x in range(initial_image.width) if (lambda c:c[2]>145 and c[0]<100 and 100<c[1]<180)(initial_image.getpixel((x,y)))]
            blue_center=((min(x for x,y in blue)+max(x for x,y in blue))/2,(min(y for x,y in blue)+max(y for x,y in blue))/2)
            drag((20,20),(initial_image.width-20,initial_image.height-20));record('marquee selects both','2 selected' in title())
            pointer(20,20);command('xdotool','click','1');time.sleep(.15);record('empty click clears','0 selected' in title())
            pointer(*center);command('xdotool','click','1');time.sleep(.15);record('single selection','1 selected' in title());selected=capture('selected')
            drag(center,(center[0]+50,center[1]+30));record('move commits','modified' in title());capture('moved')
            command('xdotool','key','ctrl+z');time.sleep(.15);capture('undo-move')
            drag((right,bottom),(right+80,bottom+30));resized=capture('resized');record('resize expands without ratio drift',bounds(resized)[2]>right+40);command('xdotool','key','ctrl+z');time.sleep(.15)
            # Dedicated visible rotation affordance avoids WM Alt interception.
            drag((center[0],top-26*scale),(center[0]+100,top+40));rotated=capture('rotated');record('rotation handle changes geometry',bounds(rotated)[1]<top-20);command('xdotool','key','ctrl+z');time.sleep(.15)
            command('xdotool','key','ctrl+alt+shift+c');time.sleep(.15);record('crop mode','crop' in title())
            drag((right,center[1]),(right-100,center[1]));crop=capture('cropped');record('crop removes right pixels',bounds(crop)[2]<right-70)
            command('xdotool','key','ctrl+z');command('xdotool','key','ctrl+alt+shift+c');time.sleep(.15)
            drag(center,(center[0]+100,center[1]+20),cancel=True);cancelled=capture('cancelled');record('Escape restores visible geometry',bounds(cancelled)==bounds(selected))
            pointer(*blue_center);command('xdotool','keydown','shift');command('xdotool','click','1');command('xdotool','keyup','shift');time.sleep(.15);record('Shift adds second image','2 selected' in title())
            pointer(*blue_center);command('xdotool','keydown','shift');command('xdotool','click','1');command('xdotool','keyup','shift');time.sleep(.15);record('Shift toggles second image off','1 selected' in title())
            # Ordinary navigation over a selected object must not commit an edit.
            pointer(*center);command('xdotool','mousedown','2');pointer(center[0]+40,center[1]+20);command('xdotool','mouseup','2');panned=capture('panned');record('middle pans over selection',bounds(panned)[0]>left+25)
            pointer(*center);command('xdotool','click','4');zoomed=capture('zoomed');record('wheel zoom changes coverage',bounds(zoomed)[2]-bounds(zoomed)[0]>bounds(panned)[2]-bounds(panned)[0])
            # Return to the initial camera using image focus; durable state is unchanged.
            pointer(*center);command('xdotool','click','--repeat','2','--delay','90','1');time.sleep(.2)
            focused_bounds=bounds(capture('navigation-focused'));left,top,right,bottom=focused_bounds;center=((left+right)/2,(top+bottom)/2)
            command('xdotool','key','ctrl+a');time.sleep(.15);record('select all','2 selected' in title())
            command('xdotool','key','alt+shift+h');command('xdotool','key','alt+shift+v');command('xdotool','key','alt+t');capture('multi-flip-filter')
            command('xdotool','key','Delete');capture('deleted');record('delete clears selection','0 selected' in title())
            command('xdotool','key','ctrl+z');time.sleep(.15);capture('undo-delete')
            pointer(*center);command('xdotool','click','1');drag(center,(center[0]-80,center[1]),mods='ctrl+alt+shift');capture('opacity')
            command('xdotool','key','ctrl+s');time.sleep(.4);record('save acknowledges committed generation','saved' in title())
            pointer(*center);command('xdotool','click','--repeat','2','--delay','90','1');capture('focused')
            # WM_DELETE through normal desktop shortcut, not XDestroyWindow.
            command('xdotool','key','alt+F4');process.wait(timeout=4)
        finally:
            if hidden.exists():hidden.rename(root/'a.png')
            command('xdotool','keyup','ctrl','alt','shift','super')
            if process.poll() is None:
                process.terminate()
                try:process.wait(timeout=2)
                except subprocess.TimeoutExpired:process.kill();process.wait()
    print(json.dumps({'returncode':process.returncode,'checks':checks},indent=2))
    if process.returncode:raise RuntimeError('native check exited unsuccessfully')


if __name__=='__main__':main()
