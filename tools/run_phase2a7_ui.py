#!/usr/bin/env python3
"""Phase2A7 serial native interaction evidence on an owned software-Vulkan display."""
import argparse, copy, json, os, subprocess, time
from pathlib import Path
from PIL import Image, ImageChops
from run_local_production import Session, wait
from run_native_image_checks import command
from run_native_annotation_checks import saved_objects
from run_image_interaction import digest
from run_phase2a_local_regression import snapshot, delta
from run_idle import close_owned_window


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for n in ('binary','profile','output'):p.add_argument('--'+n,type=Path,required=True)
    a=p.parse_args();root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
    binary=a.binary.resolve();base=json.loads(a.profile.read_text());checks=[];s=None
    xroot=Path(__file__).resolve().parents[1]/'target/native/tools/xvfb-root'
    os.environ.update(DISPLAY=':106',TACK_NATIVE_NO_WM='1',WINIT_X11_SCALE_FACTOR='1',
        LD_LIBRARY_PATH=str(xroot/'usr/lib/x86_64-linux-gnu'),
        VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',WGPU_BACKEND='vulkan')
    log=(root/'xvfb.log').open('w');x=subprocess.Popen([str(xroot/'usr/bin/Xvfb'),':106','-screen','0','1280x1024x24','-nolisten','tcp','-noreset'],stdout=log,stderr=subprocess.STDOUT)
    def record(n,ok,detail=None):
        checks.append(dict(name=n,observed=bool(ok),detail=detail));(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if not ok:raise AssertionError(n)
    def motion(px,py):
        command('xdotool','mousemove','--window',s.window,str(round(px)),str(round(py)));time.sleep(.08)
    def click(px,py,button=1):
        motion(px,py);command('xdotool','click',str(button));time.sleep(.2)
    def shot(n):
        s.shot(n);return Image.open(s.root/f'{s.name}-{n}.png').convert('RGB')
    def read():return json.loads((s.root/'profile/preferences.json').read_text())
    def binding(action):return [b for b in read()['keymap'] if b['action']==action]
    def close():
        s.close();return json.loads(s.report.read_text())
    def start(name,size='800x600',bar=1):
        run=root/name;run.mkdir();settings=copy.deepcopy(base)
        settings.update(ui_scale=1,status_bar=True,recent=[],local_views=[],keyset=dict(name='Default',path=None,dirty=False))
        settings['toolbar'].update(scale=bar,placement='Top',offset=0,offset_set=False,edge_position=2500,last_visible='Top')
        remaps={'F1':'About','F2':'KeymapEditor','F3':'EditToolbar','F4':'ResetAspectRatio',
                'F5':'AnnotationStyle(Fill)','F6':'AnnotationStyle(Color)',
                'F7':'SelectTool(Text)','F8':'SelectTool(Rectangle)',
                'F9':'SaveKeymap','F13':'ExportKeymap','F11':'ImportKeymap','F12':'ImportImages'}
        # The Action identifiers are deliberately validated by the production profile reader.
        settings['keymap']=[b for b in settings['keymap'] if b['action'] not in remaps.values()
            and b['control'] not in [{'LogicalKey':{'Named':key}} for key in remaps]]
        for key,action in remaps.items():settings['keymap'].append(dict(action=action,control={'LogicalKey':{'Named':key}},modifiers=0,modifier_match='Exact',trigger='Press'))
        for action,code in [('ContextIncrease','NumpadAdd'),('ContextDecrease','NumpadSubtract')]:
            settings['keymap']=[v for v in settings['keymap'] if v['action']!=action]
            settings['keymap'].append(dict(action=action,control={'Key':{'Code':code}},modifiers=0,modifier_match='Exact',trigger='Press'))
        # Exercise the bug reported by the owner: actual mouse Hold, not PanView.
        settings['keymap']=[b for b in settings['keymap'] if b['control']!={'Pointer':{'Mouse':'Middle'}}]
        settings['keymap'].append(dict(action='TemporaryTool(Pan)',control={'Pointer':{'Mouse':'Middle'}},modifiers=0,modifier_match='Any',trigger='Hold'))
        folder=run/'profile';folder.mkdir();(folder/'preferences.json').write_text(json.dumps(settings))
        return Session(binary,run,'artist',['open',board],dict(os.environ,TACK_PROFILE_DIR=str(folder),TACK_TEST_WINDOW_SIZE=size,GSETTINGS_BACKEND='memory',GIO_USE_VFS='local',GSK_RENDERER='cairo'))
    try:
        wait(lambda:subprocess.run(['xdpyinfo'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'owned Xvfb')
        image=root/'source.png';Image.new('RGB',(160,100),(45,130,185)).save(image)
        board=root/'source.tack';subprocess.run([str(binary),'create',str(board),'--embedded',str(image)],check=True,capture_output=True,timeout=20)
        for size,bar in [('800x600',1),('800x600',2),('800x600',3),('1024x768',1)]:
            s=start(f'{size}-bar{bar}',size,bar);time.sleep(.8);s.key('Escape');motion(760,520)
            plain=shot('canvas');s.key('F1');time.sleep(.5);about=shot('about')
            w,h=map(int,size.split('x'));px=(w-600)//2;py=(h-288)//2
            editable=binary.parent/'gfx/icons/work_icons/logo_tack_about.png'
            supplied=Image.open(editable if editable.is_file() else binary.parent/'tack-about-logo.png').convert('RGBA')
            portrait_width=round(286*207/224);lx=px+portrait_width+12;ly=py+8
            opaque=[(xx,yy) for yy in range(supplied.height) for xx in range(supplied.width) if supplied.getpixel((xx,yy))[3]==255]
            record(f'{size} bar{bar} provided logo exact nearest pixels',all(about.getpixel((lx+xx,ly+yy))==supplied.getpixel((xx,yy))[:3] for xx,yy in opaque),dict(logo_rect=[lx,ly,supplied.width,supplied.height]))
            record(f'{size} bar{bar} centered About and border',about.getpixel((px,py))!=plain.getpixel((px,py)))
            click(w-5,h-50);dismiss=shot('outside-dismiss')
            record(f'{size} bar{bar} About outside dismissal',ImageChops.difference(plain.crop((px,py,px+600,py+288)),dismiss.crop((px,py,px+600,py+288))).getbbox() is None)
            command('xdotool','windowsize',s.window,str(w+224),str(h+168));time.sleep(.25);shot('resize')
            report=close()
            bounds=report['chrome']['toolbar_bounds'];travel=w+224-(bounds[2]-bounds[0])
            record(f'{size} bar{bar} native toolbar stays at quarter edge after resize',abs(bounds[0]-travel*.25)<=bar,bounds)
            record(f'{size} bar{bar} independent toolbar cells and released artwork',all(r['rect'][2]-r['rect'][0]==16*bar and r['rect'][3]-r['rect'][1]==16*bar for r in report['chrome']['toolbar_cells'] if r['action']) and report['about']['image_gpu_bytes']==0,report['chrome']['toolbar_bounds']);s=None
        s=start('workflows');time.sleep(.8);s.key('Escape','F9')
        window=wait(s.picker_window,'Default Save redirects to Save As');command('xdotool','windowfocus',window);s.key('Escape');wait(lambda:not s.picker_window(),'default save cancelled');s.focus();time.sleep(.2)
        record('immutable Default Save redirects to Save As without changing profile',read()['keyset']['name']=='Default' and read()['keyset']['path'] is None)
        s.key('Escape','F3');motion(80,100);editor=shot('toolbar-editor');command('xdotool','click','5');time.sleep(.2);scrolled=shot('toolbar-editor-wheel')
        record('Edit Toolbar wheel scrolls Available independently of Order',ImageChops.difference(editor.crop((24,76,304,320)),scrolled.crop((24,76,304,320))).getbbox() is not None and ImageChops.difference(editor.crop((320,76,590,320)),scrolled.crop((320,76,590,320))).getbbox() is None)
        s.key('Escape','F2');motion(50,100)
        initial=shot('grid');motion(50,320);time.sleep(.5);hover=shot('hover')
        record('Keymap pointer resting at edge does not scroll',ImageChops.difference(initial.crop((24,108,580,300)),hover.crop((24,108,580,300))).getbbox() is None)
        command('xdotool','click','5');time.sleep(.25);wheel=shot('wheel')
        record('Keymap wheel scrolls bounded list',ImageChops.difference(initial.crop((20,84,596,326)),wheel.crop((20,84,596,326))).getbbox() is not None)
        # Thumb remains near viewport top after a few wheel increments.
        motion(600,95);command('xdotool','mousedown','1');motion(600,305);command('xdotool','mouseup','1');time.sleep(.2);thumb=shot('thumb')
        record('Keymap direct thumb drag changes viewport',ImageChops.difference(wheel.crop((20,84,596,326)),thumb.crop((20,84,596,326))).getbbox() is not None)
        s.key('Escape');s.key('F2');s.text('Undo');s.key('Right','Right','Return');time.sleep(.4)
        record('grid Behavior Enter changes Undo to Release',binding('Undo')[0]['trigger']=='Release')
        s.key('Left','Return','ctrl+v');staged=shot('collision');before=binding('Undo');s.key('Escape')
        record('shortcut collision Escape keeps previous binding',binding('Undo')==before and bool(binding('Paste')))
        s.key('Return','ctrl+v','Return');time.sleep(.4);shot('assigned')
        record('shortcut collision Enter commits reassignment and dirty marker',binding('Undo')[0]['control']=={'LogicalKey':{'Character':'v'}} and not binding('Paste') and read()['keyset']['dirty'])
        s.key('Escape','Escape');target=root/(root.name+'-artist.tackey');s.picker('F13',target);time.sleep(.4)
        record('Save As names keyset and clears dirty marker',target.is_file() and read()['keyset']['name']==target.stem and not read()['keyset']['dirty'])
        s.key('F2');s.text('Undo');s.key('Right','Right','Return');time.sleep(.3);s.key('Escape','Escape','F9');time.sleep(.5)
        record('Save overwrites loaded user keyset without picker',not s.picker_window() and not read()['keyset']['dirty'] and json.loads(target.read_text())['bindings']==read()['keymap'])
        s.key('F2');s.text('Undo');s.key('Right','Right','Return');time.sleep(.3);s.key('Escape','Escape');s.picker('F11',target,selects_file=True);time.sleep(.4)
        record('Load restores saved keyset identity and bindings',bytes(read()['keyset']['path']['bytes']).decode()==str(target) and not read()['keyset']['dirty'] and json.loads(target.read_text())['bindings']==read()['keymap'])
        s.key('Escape')
        # Bookmark changes only local profile, including Right Mouse priority over popup fallback.
        before=digest(board);s.key('b');shot('bookmark');click(400,280,3);shot('bookmark-staged');s.key('Return');time.sleep(.4)
        record('B captures arbitrary Right Mouse shortcut and stays local',len(read()['local_views'])==1 and bool(binding('JumpCameraSlot(0)')) and digest(board)==before)
        motion(400,300);command('xdotool','mousedown','2');motion(480,350);command('xdotool','mouseup','2');time.sleep(.3);panned=shot('panned');click(400,300,3);recalled=shot('recalled')
        record('Middle Hold Pan begins same press and Right Mouse recalls view',ImageChops.difference(panned.crop((100,100,700,540)),recalled.crop((100,100,700,540))).getbbox() is not None)
        # Note/Rectangle actual native input and persistence.
        s.key('F7');click(520,420);s.text('Native note');s.key('shift+Return');s.text('Second line');s.key('Return','KP_Add','ctrl+s');time.sleep(.5)
        doc=saved_objects(board);notes=[o for o in doc['objects'] if o['kind']==3];shot('note')
        record('Note Enter keeps paper and Shift Enter newline',bool(notes) and notes[-1]['text']=='Native note\nSecond line' and notes[-1]['fill'] is not None and notes[-1]['font_size']==32,notes[-1] if notes else None)
        s.key('F8');motion(200,400);command('xdotool','mousedown','1');motion(320,475);command('xdotool','mouseup','1');s.key('F5','F6','F5','ctrl+s');time.sleep(.4)
        doc=saved_objects(board);rects=[o for o in doc['objects'] if o['kind']==4];shot('rectangle')
        record('Rectangle fill Color Cycle persists both hues and opacity',bool(rects) and rects[-1]['fill'][:3]==rects[-1]['stroke'][:3] and rects[-1]['fill'][3]==191,rects[-1] if rects else None)
        board_before=digest(board)
        for key in ('ctrl+shift+s','ctrl+o','F12','F11','F13'):
            s.key(key);window=wait(s.picker_window,'owned cancel picker');command('xdotool','windowfocus',window);s.key('Escape');wait(lambda:not s.picker_window(),'picker cancelled');s.focus();time.sleep(.2);shot('cancel-'+key.replace('+','-'))
            record('native '+key+' cancel retains document and opens no Error',s.process.poll() is None and digest(board)==board_before and 'error' not in s.title().lower())
            s.key('Escape')
        motion(700,550);time.sleep(2);before=snapshot(s.process.pid);started=time.monotonic();time.sleep(3);after=snapshot(s.process.pid);idle=delta(before,after,time.monotonic()-started)
        report=close();redraws=sum(started-s.started<=f['elapsed_ms']/1000<=started-s.started+idle['seconds'] for f in report['frames'])
        record('settled local idle has zero redraw I/O TCP/UDP and no caret timer',redraws==0 and not any(idle['io_delta'].values()) and all(r['kind']=='unix' for r in idle['socket_records']) and not report['chrome']['caret_deadline_active'],dict(redraws=redraws,threads=idle['threads'],rss=idle['rss_after'],io=idle['io_delta']))
        s=None
        normal=root/'public-title';normal.mkdir()
        s=Session(binary,normal,'normal',[],dict(os.environ,TACK_PROFILE_DIR=str(normal/'profile'),TACK_TEST_WINDOW_SIZE='800x600'))
        time.sleep(.4);initial_title=s.title();s.key('r');motion(200,200);command('xdotool','mousedown','1');motion(300,300);command('xdotool','mouseup','1');time.sleep(.2);dirty_title=s.title();shot('normal-title')
        record('public title contains only Tack file identity and dirty marker',initial_title.startswith('Tack — ') and '*' not in initial_title and dirty_title==initial_title+' *',dict(initial=initial_title,dirty=dirty_title))
        close_owned_window(s.window);time.sleep(.2);confirm=shot('unsaved-confirmation');click(790,540);outside=shot('unsaved-outside')
        record('outside click cannot dismiss unsaved-data confirmation',s.process.poll() is None and ImageChops.difference(confirm.crop((160,180,640,420)),outside.crop((160,180,640,420))).getbbox() is None)
        s.key('Escape');s.close(discard=True);s=None
        (root/'receipt.json').write_text(json.dumps(dict(binary_sha256=digest(binary),harness_sha256=digest(__file__),scope='Owned Linux Xvfb / software Vulkan; physical artist, Windows desktop and two-computer LAN pending',checks=checks,idle=idle),indent=2)+'\n')
        print(root/'receipt.json')
    finally:
        if s:s.kill()
        x.terminate();x.wait(timeout=5);log.close()
if __name__=='__main__':main()
