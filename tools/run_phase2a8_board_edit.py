#!/usr/bin/env python3
"""Serial native Scribble/Frame checks on an owned X11/software-Vulkan display."""
import argparse, json, os, subprocess, time
from pathlib import Path
from PIL import Image
from run_local_production import Session, wait
from run_native_image_checks import command
from run_native_annotation_checks import saved_objects
from run_image_interaction import digest

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('binary','profile','output'):p.add_argument('--'+name,type=Path,required=True)
    a=p.parse_args();binary=a.binary.resolve();root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
    xroot=Path(__file__).resolve().parents[1]/'target/native/tools/xvfb-root'
    os.environ.update(DISPLAY=':110',TACK_NATIVE_NO_WM='1',WINIT_X11_SCALE_FACTOR='1',LP_NUM_THREADS='2',
        LD_LIBRARY_PATH=str(xroot/'usr/lib/x86_64-linux-gnu'),
        VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',WGPU_BACKEND='vulkan')
    log=(root/'xvfb.log').open('w');x=subprocess.Popen([str(xroot/'usr/bin/Xvfb'),':110','-screen','0','1280x1024x24','-nolisten','tcp','-noreset'],stdout=log,stderr=subprocess.STDOUT)
    s=None;checks=[]
    def record(name,ok,detail=None):
        checks.append(dict(name=name,observed=bool(ok),detail=detail));(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if not ok:raise AssertionError(name)
    def motion(px,py):
        command('xdotool','mousemove','--window',s.window,str(round(px)),str(round(py)));time.sleep(.04)
    def click(px,py):motion(px,py);command('xdotool','click','1');time.sleep(.15)
    def drag(start,end):
        motion(*start);command('xdotool','mousedown','1')
        for k in range(1,9):motion(*(start[j]+(end[j]-start[j])*k/8 for j in range(2)))
        command('xdotool','mouseup','1');time.sleep(.15)
    def save():return s.save(board)
    def one(data,kind):return next(o for o in data['objects'] if o['kind']==kind)
    try:
        wait(lambda:subprocess.run(['xdpyinfo'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'owned Xvfb')
        image=root/'fixture.png';Image.new('RGB',(160,100),(40,120,190)).save(image)
        board=root/'board.tack';subprocess.run([str(binary),'create',str(board),'--embedded',str(image)],check=True,capture_output=True,timeout=20)
        settings=json.loads(a.profile.read_text());settings.update(recent=[],local_views=[],status_bar=True)
        settings['toolbar'].update(scale=1,placement='Top',edge_position=0)
        settings['toolbar']['actions'].insert(7,'LinkToFrame')
        remaps={'F1':'LinkToFrame','F2':'UnlinkFromFrame','F3':'SelectLinkedObjects','F4':'FinishScribble','F5':'MergeScribbles','F6':'SelectTool(Eraser)'}
        settings['keymap']=[b for b in settings['keymap'] if b['action'] not in remaps.values() and b['control'] not in [{'LogicalKey':{'Named':key}} for key in remaps]]
        for key,action in remaps.items():settings['keymap'].append(dict(action=action,control={'LogicalKey':{'Named':key}},modifiers=0,modifier_match='Exact',trigger='Press'))
        profile=root/'profile';profile.mkdir();(profile/'preferences.json').write_text(json.dumps(settings))
        s=Session(binary,root,'board-edit',['open',board],dict(os.environ,TACK_PROFILE_DIR=str(profile),TACK_TEST_WINDOW_SIZE='800x600'))
        time.sleep(1);s.key('Escape');click(750,550);s.key('ctrl+shift+f');framed=save();f=one(framed,'frame')
        record('Frame created for native interaction',f['size'][0]>0)
        s.key('p');drag((200,200),(400,220));drag((200,260),(400,260));s.shot('two-strokes-live')
        s.key('F4');drawn=save();sc=one(drawn,9)
        record('two pointer strokes become one compound Scribble',len([o for o in drawn['objects'] if o['kind']==9])==1 and len(sc['strokes'])==2,sc)
        s.key('F1');motion(600,400);s.shot('link-dots');click(600,400);linked=save()
        record('selected-object Link commits parent relation',linked['schema']==7 and linked['links']==[[sc['id'],f['id']]],linked['links'])
        s.key('ctrl+z');unlinked=save();record('Link undo removes only relation',unlinked==drawn)
        s.key('ctrl+y');record('Link redo exact',save()==linked)
        s.key('v');click(160,300);drag((160,300),(190,320));moved=save();mf=one(moved,'frame');ms=one(moved,9)
        delta=lambda new,old:[round(new['center'][j]-old['center'][j],6) for j in range(2)]
        record('native Frame move translates linked Scribble exactly',delta(mf,f)==[30,20] and delta(ms,sc)==[30,20],dict(frame=delta(mf,f),child=delta(ms,sc)))
        drag((670,500),(690,520));resized=save()
        record('native Frame resize leaves child unchanged',one(resized,'frame')['size']!=mf['size'] and one(resized,9)==ms)
        s.key('Delete');deleted=save();record('delete Frame preserves child and clears links',not any(o['kind']=='frame' for o in deleted['objects']) and one(deleted,9)==ms and deleted.get('links',[])==[])
        s.key('ctrl+z');record('delete Frame undo restores relation and pose exactly',save()==resized)
        click(330,230);s.key('F2');unlinked=save();record('Unlink preserves child position',unlinked.get('links',[])==[] and one(unlinked,9)==ms)
        s.key('ctrl+z');record('Unlink undo exact',save()==resized)
        # Matching parent permits merge while preserving color/width/opacity.
        s.key('p');drag((220,410),(420,410));s.key('F4');second=save();new=next(o for o in second['objects'] if o['kind']==8)
        s.key('F1');click(600,400);both=save();record('second Scribble links independently',len(both['links'])==2)
        s.key('v');command('xdotool','keydown','shift');click(330,230);command('xdotool','keyup','shift');s.key('F5');merged=save();merged_sc=one(merged,9)
        record('Merge creates one Scribble with three independent strokes',len([o for o in merged['objects'] if o['kind'] in (8,9)])==1 and len(merged_sc['strokes'])==3)
        s.key('ctrl+z');record('Merge undo exact',save()==both);s.key('ctrl+y');record('Merge redo exact',save()==merged)
        s.key('F6');drag((330,200),(330,310));erased=save();es=one(erased,9)
        record('target-only swept Eraser splits Scribble segments',es!=merged_sc and len(es['strokes'])>3 and one(erased,'image')==one(merged,'image') and one(erased,'frame')==one(merged,'frame'))
        s.key('ctrl+z');record('Eraser undo exact',save()==merged);s.key('ctrl+y');record('Eraser redo exact',save()==erased)
        s.shot('final');s.close();report=json.loads(s.report.read_text());s=None
        subprocess.run([str(binary),'inspect',str(board)],check=True,capture_output=True,timeout=10)
        record('schema7 save/reopen retains authoring exactly',saved_objects(board)==erased)
        (root/'receipt.json').write_text(json.dumps(dict(checks=len(checks),passed=all(c['observed'] for c in checks),binary_sha256=digest(binary),harness_sha256=digest(__file__),build_info=json.loads(subprocess.check_output([str(binary),'--build-info'])),report=report),indent=2)+'\n')
    finally:
        if s:s.kill()
        x.terminate();x.wait(timeout=5);log.close()
if __name__=='__main__':main()
