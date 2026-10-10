#!/usr/bin/env python3
"""Accelerated Mulot deadline across real Save As/rebase and Close Board."""
import argparse, json, os, struct, subprocess, time
from pathlib import Path
from run_local_production import Session, wait
from run_native_image_checks import command
from run_native_annotation_checks import saved_objects


def linked_sources(path):
    """Read source descriptors from the same owned schema-8 fixture as objects."""
    data = path.read_bytes()
    count = struct.unpack_from('<I', data, 48)[0]
    offset = 96
    result = []
    for _ in range(count):
        length = struct.unpack_from('<I', data, offset)[0]
        record = data[offset + 4:offset + 4 + length]
        if struct.unpack_from('<H', record)[0] != 1:
            raise ValueError('source descriptor version')
        revision = struct.unpack_from('<Q', record, 18)[0]
        cursor = 27 + (20 if record[26] else 0)
        if record[cursor] in (1, 2):
            size = struct.unpack_from('<I', record, cursor + 2)[0]
            result.append(dict(revision=revision, absolute=bool(record[cursor + 1]),
                               path=os.fsdecode(record[cursor + 6:cursor + 6 + size])))
        offset += 4 + length
    return result


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('binary','profile','output'):p.add_argument('--'+name,type=Path,required=True)
    a=p.parse_args();binary=a.binary.resolve();root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
    repo=Path(__file__).resolve().parents[1]; xr=repo/'target/native/tools/xvfb-root'
    fixture_env=dict(os.environ,TACK_MOUSE_FIXTURE_ROOT=str(root))
    subprocess.run(['cargo','test','--locked','-p','tack-app','--test','local_production','mouse_lifecycle_relative_fixture','--','--ignored'],cwd=repo,env=fixture_env,check=True,stdout=(root/'fixture-build.log').open('w'),stderr=subprocess.STDOUT,timeout=60)
    env=dict(os.environ,DISPLAY=':122',TACK_NATIVE_NO_WM='1',TACK_NATIVE_DIAGNOSTICS='1',TACK_TEST_WINDOW_SIZE='800x600',TACK_TEST_MOUSE_EASTER_MS='1500',WINIT_X11_SCALE_FACTOR='1',LP_NUM_THREADS='2',LD_LIBRARY_PATH=str(xr/'usr/lib/x86_64-linux-gnu'),VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',WGPU_BACKEND='vulkan')
    os.environ.update(env);log=(root/'xvfb.log').open('w');x=subprocess.Popen([str(xr/'usr/bin/Xvfb'),':122','-screen','0','1280x1024x24','-nolisten','tcp','-noreset'],env=env,stdout=log,stderr=subprocess.STDOUT)
    s=None;checks=[]
    def record(name,ok,detail=None):
        checks.append(dict(name=name,observed=bool(ok),detail=detail));(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if not ok:raise AssertionError(name)
    def profile(name):
        settings=json.loads(a.profile.read_text());settings.update(recent=[],local_views=[],status_bar=True)
        remaps={'F1':'SelectTool(Mouse)','F2':'SelectTool(Pointer)','F3':'SaveAs','F4':'CloseBoard'}
        settings['keymap']=[b for b in settings['keymap'] if b['action'] not in remaps.values() and b['control'] not in [{'LogicalKey':{'Named':key}} for key in remaps]]
        for key,action in remaps.items():settings['keymap'].append(dict(action=action,control={'LogicalKey':{'Named':key}},modifiers=0,modifier_match='Exact',trigger='Press'))
        path=root/name;path.mkdir();(path/'preferences.json').write_text(json.dumps(settings));return path
    try:
        wait(lambda:subprocess.run(['xdpyinfo'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'owned display')
        board=root/'relative.tack';out=root/'relocated';out.mkdir();target=out/'saved.tack'
        s=Session(binary,root,'save-as',['open',board],dict(env,TACK_PROFILE_DIR=str(profile('save-profile'))));time.sleep(.7);s.key('Escape','F1','F3')
        picker=wait(s.picker_window,'owned Save As picker');time.sleep(2.)
        record('deadline waits during Save As chooser', 'saved' in s.title().lower() and 'dirty' not in s.title().lower(), s.title())
        command('xdotool','windowfocus',picker);s.key('ctrl+l','ctrl+a');s.text(str(target));s.key('Return');time.sleep(.5)
        if s.picker_window():s.key('alt+o')
        wait(lambda:not s.picker_window(),'Save As picker accepted');s.focus();time.sleep(1.)
        record('relocated saved snapshot precedes deferred insertion',target.is_file() and len(saved_objects(target)['objects'])==1)
        # The source path conversion explicitly exercises reset-history Save As.
        command('xdotool','mousemove','--window',s.window,'700','400');s.key('F2');time.sleep(.2)
        inserted=s.save(target);record('one egg admitted after Save As reset',len(inserted['objects'])==2)
        s.key('ctrl+z');undone=s.save(target);record('deferred insertion is a fresh one-step Undo after reset',len(undone['objects'])==1)
        sources = linked_sources(target)
        record('Save As rebases original relative source', linked_sources(board)[0]['absolute'] is False and any(source['revision'] > 1 and source['absolute'] and source['path'] == str(root/'fixture.png') for source in sources), sources)
        s.close();report=json.loads(s.report.read_text());s=None
        inspection=subprocess.check_output([str(binary),'inspect',str(target)],text=True);(root/'save-as-inspect.txt').write_text(inspection)
        # Close Board while continuous activation is armed; replacement must cancel it.
        s=Session(binary,root,'close-board',['open',board],dict(env,TACK_PROFILE_DIR=str(profile('close-profile'))));time.sleep(.7);s.key('Escape','F1','F4');time.sleep(2.2)
        record('Close Board replaces with empty board without egg leak','0 selected' in s.title() and 'Untitled' in s.title(),s.title())
        s.close();closed=json.loads(s.report.read_text());s=None
        record('replacement cancels deadline and has no image insertion',closed['mouse']['deadline_active'] is False and closed['editing']['undo_entries']==0 and closed['editing']['generation']==0,closed['mouse'])
        # Dirty Close Board -> Cancel must leave the board usable on reactivation.
        cancelled_board=root/'cancelled-close.tack';cancelled_board.write_bytes(board.read_bytes())
        s=Session(binary,root,'cancel-close',['open',cancelled_board],dict(env,TACK_PROFILE_DIR=str(profile('cancel-profile'))));time.sleep(.7)
        s.key('Escape','ctrl+a','ctrl+d','F4');time.sleep(.2);s.key('Escape','F1');time.sleep(2.)
        s.key('F2');after_cancel=s.save(cancelled_board)
        record('dirty Close Board cancellation allows future Mouse admission',len(after_cancel['objects'])==3,after_cancel)
        s.close();cancelled=json.loads(s.report.read_text());s=None
        record('cancelled Close Board activation inserts only once',cancelled['mouse']['spawn_requests']==1 and cancelled['mouse']['deadline_active'] is False,cancelled['mouse'])
        (root/'receipt.json').write_text(json.dumps(dict(passed=all(c['observed'] for c in checks),checks=len(checks),save_as=report,close_board=closed,cancelled_close=cancelled),indent=2)+'\n')
    finally:
        if s:s.kill()
        x.terminate();x.wait(timeout=5);log.close()
if __name__=='__main__':main()
