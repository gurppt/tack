#!/usr/bin/env python3
"""2A6 owned-display native pixel/workflow evidence; never touches artist data."""
import argparse, copy, json, os, subprocess, time
from pathlib import Path
from PIL import Image, ImageChops
from run_local_production import Session
from run_native_image_checks import command
from run_native_annotation_checks import saved_objects
from run_image_interaction import digest
from run_phase2a_local_regression import snapshot, delta


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ('binary','profile','output'):parser.add_argument('--'+name,type=Path,required=True)
    args=parser.parse_args()
    root=args.output.resolve();root.mkdir(parents=True,exist_ok=False)
    binary=args.binary.resolve();base=json.loads(args.profile.read_text())
    xroot=Path(__file__).resolve().parents[1]/'target/native/tools/xvfb-root'
    os.environ.update(DISPLAY=':106',TACK_NATIVE_NO_WM='1',WINIT_X11_SCALE_FACTOR='1',
        LD_LIBRARY_PATH=str(xroot/'usr/lib/x86_64-linux-gnu'))
    log=(root/'xvfb.log').open('w')
    x=subprocess.Popen([str(xroot/'usr/bin/Xvfb'),':106','-screen','0','1280x1024x24','-nolisten','tcp','-noreset'],stdout=log,stderr=subprocess.STDOUT)
    checks=[];session=None
    def record(name,passed,detail=None):
        checks.append(dict(name=name,observed=bool(passed),detail=detail))
        (root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if not passed:raise AssertionError(name)
    def pointer(x,y):
        command('xdotool','mousemove','--window',session.window,str(round(x)),str(round(y)))
        time.sleep(.06)
    def click(x,y,button=1,count=1):
        pointer(x,y);command('xdotool','click','--repeat',str(count),'--delay','100',str(button));time.sleep(.15)
    def capture(name):
        session.shot(name)
        return Image.open(session.root/f'{session.name}-{name}.png').convert('RGB')
    def close():
        session.close();return json.loads(session.report.read_text())
    def profile(run,placement,scale):
        settings=copy.deepcopy(base)
        settings.update(recent=[],local_views=[],ui_scale=scale,status_bar=False)
        settings['toolbar'].update(placement=placement,offset=0,offset_set=False,last_visible=placement,floating=[48,48])
        actions=list(base['toolbar']['actions']);actions.insert(7,'Separator');actions.insert(11,'Separator')
        settings['toolbar']['actions']=actions
        remaps={'F2':'RenameFrame','F3':'AnnotationStyle(Color)','F4':'EditToolbar','F6':'CreateFrame','F8':'ToggleToolbar'}
        settings['keymap']=[b for b in settings['keymap'] if b['action'] not in remaps.values()
            and b['control'] not in [{'LogicalKey':{'Named':key}} for key in remaps]]
        for key,action in remaps.items():
            settings['keymap'].append(dict(action=action,control={'LogicalKey':{'Named':key}},modifiers=0,modifier_match='Exact',trigger='Press'))
        path=run/'profile';path.mkdir(parents=True);(path/'preferences.json').write_text(json.dumps(settings))
        return dict(os.environ,TACK_PROFILE_DIR=str(path),TACK_TEST_WINDOW_SIZE='800x600',TACK_NATIVE_DIAGNOSTICS='1')
    def bar_rect(placement,scale,w,h):
        length=210*scale;side=16*scale
        if placement in ('Top','Bottom'):x=round((w/scale-210)/2)*scale;y=0 if placement=='Top' else h-side;return (x,y,x+length,y+side)
        if placement in ('Left','Right'):x=0 if placement=='Left' else w-side;y=round((h/scale-210)/2)*scale;return (x,y,x+side,y+length)
        return (48*scale,48*scale,48*scale+length,48*scale+side)
    try:
        for _ in range(50):
            if subprocess.run(['xdpyinfo'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0:break
            time.sleep(.1)
        image=root/'image.png';Image.new('RGB',(160,100),(45,130,185)).save(image)
        board=root/'source.tack'
        subprocess.run([str(binary),'create',str(board),'--embedded',str(image)],check=True,capture_output=True,timeout=20)
        for size,scale in [('800x600',1),('800x600',2),('1024x768',1)]:
            w,h=map(int,size.split('x'))
            for placement in ('Top','Bottom','Left','Right','Floating'):
                name=f'{size}-{scale}x-{placement}';run=root/name;run.mkdir()
                env=profile(run,placement,scale);env['TACK_TEST_WINDOW_SIZE']=size
                session=Session(binary,run,'artist',['open',board],env)
                time.sleep(.65);session.key('Escape');pointer(w-70,h-90)
                before=capture('bar');rect=bar_rect(placement,scale,w,h)
                # Context selection legitimately changes command enabled states.
                # Compare Pointer/Pan pixels, whose state is independent of it.
                x1,y1,x2,y2=rect
                rect=(x1,y1+16*scale,x2,y1+48*scale) if placement in ('Left','Right') else (x1+16*scale,y1,x1+48*scale,y2)
                # Canvas and object menu, repeated right-click, Escape.
                same=True
                canvas=(40 if placement=='Right' else w-40,h-150)
                obj=(w//4 if placement=='Right' else w//2,h//2)
                for px,py in [canvas,obj,canvas]:
                    click(px,py,3);shown=capture('menu')
                    same &= ImageChops.difference(before.crop(rect),shown.crop(rect)).getbbox() is None
                    session.key('Down','Up','Escape')
                    dismissed=capture('dismissed')
                    same &= ImageChops.difference(before.crop(rect),dismissed.crop(rect)).getbbox() is None
                record(f'context leaves {name} bar unchanged',same)
                session.key('F4');capture('editor')
                session.key('Right','Tab','Right','Right','Right','Right','Return') # Position control.
                session.key('Escape')
                report=close();record(f'{name} exact pixel cells and no gizmo rectangle',
                    all((r['rect'][2]-r['rect'][0],r['rect'][3]-r['rect'][1])==(16*scale,16*scale)
                        for r in report['chrome']['toolbar_cells'] if r['action'])
                    and report['chrome']['selection_outline_count']==0)
                session.kill();session=None
        # Full keyboard editor workflow, menu entry, Frame rename/color/caret.
        run=root/'workflows';run.mkdir();env=profile(run,'Top',1)
        session=Session(binary,run,'artist',['open',board],env);time.sleep(.7)
        session.key('Escape','F10','Down','Down','Down','Down','Right','Return')
        capture('tools-editor')
        # Available Separator -> Add -> selects last Order row -> reorder -> Remove.
        session.key('Left','Home','Up','Up','Up','Up','Up','Up','Up','Up','Up','Up','Return')
        time.sleep(.3)
        stored=lambda:json.loads((run/'profile/preferences.json').read_text())['toolbar']['actions']
        record('keyboard adds Separator and persists it',stored()[-1]=='Separator')
        capture('added-highlight');session.key('ctrl+Up');capture('reordered')
        record('Ctrl+Up reorders selected Separator',stored()[-2]=='Separator')
        session.key('Tab','Right','Return');record('keyboard Remove preserves nearest Order item',len(stored())==14)
        session.key('Tab','Right','Right','Right','Right','Right','Right','Return') # Done.
        session.key('Escape');time.sleep(1.3)
        session.key('ctrl+a','F6','F2');session.text('Frame name');capture('rename-caret');session.key('Return','ctrl+s')
        time.sleep(.4);frames=[o for o in saved_objects(board)['objects'] if o['kind']=='frame']
        record('F2 Frame rename validates without Unicode caret in document',frames and frames[-1]['name']=='Frame name')
        normal=capture('frame-gray')
        # Label is the solid Frame-color strip above the outer frame. Find a
        # horizontal filled run rather than assuming camera position or font.
        # Find the light gray label run; image and UI backgrounds have unequal RGB.
        label=None
        for y in range(18,580):
            runs=[];start=None
            for px in range(20,780):
                rgb=normal.getpixel((px,y));gray=max(rgb)-min(rgb)<16 and 100<sum(rgb)/3<210
                if gray and start is None:start=px
                elif not gray and start is not None:
                    if px-start>100:runs.append((start,px))
                    start=None
            if runs:
                label=(runs[0][0]+10,y);break
        record('Frame colored label visible',label is not None,label)
        click(*label,count=2);capture('double-click-rename');session.text('Double click name');session.key('Return','ctrl+s');time.sleep(.3)
        frames=[o for o in saved_objects(board)['objects'] if o['kind']=='frame']
        record('double-click label shares semantic rename path',frames[-1]['name']=='Double click name')
        for i in range(8):session.key('F3');capture(f'frame-color-{i}')
        session.key('F2');session.text('Canceled');session.key('Escape','ctrl+s');time.sleep(.3)
        frames=[o for o in saved_objects(board)['objects'] if o['kind']=='frame']
        record('Escape cancels Frame draft',frames[-1]['name']=='Double click name')
        pointer(700,550);time.sleep(2)
        before=snapshot(session.process.pid);started=time.monotonic();time.sleep(3);after=snapshot(session.process.pid);seconds=time.monotonic()-started
        report=close();idle=delta(before,after,seconds)
        redraws=sum(started-session.started<=f['elapsed_ms']/1000<=started-session.started+seconds for f in report['frames'])
        record('UI settles without caret timer/redraw/I/O/network',redraws==0 and not report['chrome']['caret_deadline_active']
            and not any(idle['io_delta'].values()) and all(s['kind']=='unix' for s in idle['socket_records']),dict(redraws=redraws,threads=idle['threads'],rss=idle['rss_after'],io=idle['io_delta']))
        (root/'receipt.json').write_text(json.dumps(dict(binary_sha256=digest(binary),harness_sha256=digest(__file__),checks=checks,idle=idle,scope='Owned Linux Xvfb on one physical GPU; human and physical Windows acceptance pending'),indent=2)+'\n')
        print(root/'receipt.json')
    finally:
        if session:session.kill()
        x.terminate();x.wait(timeout=5);log.close()

if __name__=='__main__':main()
