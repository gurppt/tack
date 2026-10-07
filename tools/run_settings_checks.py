#!/usr/bin/env python3
"""Owned X11 mouse/keyboard Preferences and Keymap proof at 800x600, 1x/2x."""
import argparse
import json
import os
from pathlib import Path
import time
from PIL import Image
from run_local_production import Session, wait
from run_native_image_checks import command
from run_idle import observe
from run_image_interaction import digest


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True)
    p.add_argument('--profile-seed',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args()
    if os.environ.get('DISPLAY') in (None,':0',':0.0'):p.error('owned isolated display required')
    root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
    rows=[];seed=json.loads(a.profile_seed.read_text())
    for theme in ('VeryDark','NeutralGray','Light'):
        for scale in (1,2):
            work=root/f'{theme}-{scale}x';work.mkdir();profile=work/'profile';profile.mkdir()
            initial=dict(seed,theme=theme,ui_scale=scale,recent=[])
            path=profile/'preferences.json';path.write_text(json.dumps(initial))
            layout='fr' if theme=='Light' else 'us';command('setxkbmap','-layout',layout)
            env=dict(os.environ,TACK_PROFILE_DIR=str(profile),TACK_TEST_WINDOW_SIZE='800x600',TACK_NATIVE_NO_WM='1',
                     GSETTINGS_BACKEND='memory',GIO_USE_VFS='local',GSK_RENDERER='cairo')
            s=Session(a.binary.resolve(),work,'settings',['new',work/'board.tack'],env)
            checks=[]
            def read():return json.loads(path.read_text())
            def check(name,valid):
                checks.append({'name':name,'passed':bool(valid)});(work/'checks.json').write_text(json.dumps(checks,indent=2))
                if not valid:raise AssertionError(str(work)+': '+name)
            def click(x,y):
                command('xdotool','mousemove','--window',s.window,int(x*scale),int(y*scale));command('xdotool','click','1');time.sleep(.12)
            width=min(600,800/scale-24);height=min(440,600/scale-24)
            footer=12+height-114;cell=(width-32)/3
            def button(col,row=0):click(24+col*(cell+4)+cell/2,footer+row*26+11)
            def pref(row):
                s.key('ctrl+comma');s.key(*(['Down']*row)) if row else None
            def search(text):click(80,52);s.text(text)
            def undo():return [b for b in read()['keymap'] if b['action']=='Undo']
            def idle(name):
                time.sleep(2.);begin=time.monotonic();old=observe(s.process.pid);time.sleep(.8);new=observe(s.process.pid);end=time.monotonic()
                rows.append({'case':work.name,'state':name,'begin_ms':(begin-s.started)*1000,'end_ms':(end-s.started)*1000,
                    'rss_delta':new['rss_bytes']-old['rss_bytes'],'rss_bytes':new['rss_bytes'],
                    'thread_count':len(new['tasks']),'thread_delta':len(new['tasks'])-len(old['tasks']),
                    'ticks':sum(new['tasks'].get(t,v)['ticks']-v['ticks'] for t,v in old['tasks'].items()),
                    'io':{k:new['io'][k]-old['io'][k] for k in old['io']}})
                time.sleep(.2) # no next input inside the conservative clock-margin interval
            def pick(path,importing):
                dialog=wait(s.picker_window,'owned keymap picker')
                command('xdotool','windowfocus','--sync',dialog);s.key('ctrl+l','ctrl+a')
                s.text(str(path.parent)+'/' if importing else str(path));s.key('Return');time.sleep(.6)
                if importing and s.picker_window():
                    s.key('ctrl+f');s.text(path.name);time.sleep(.4)
                    if s.picker_window():
                        command('xdotool','mousemove','--window',dialog,250,94);command('xdotool','click','--repeat','2','--delay','100','1')
                if s.picker_window():s.key('alt+o')
                wait(lambda:not s.picker_window(),'picker accepted',20);s.focus();time.sleep(.4)
            try:
                for row,field in ((5,'handle_size'),(6,'hit_radius')):
                    pref(row);visible=int((12+height-32-48)//22);first=min(max(0,row-visible//2),max(0,11-visible));y=48+(row-first)*22+11
                    old=read()[field];click(width-120,y);wait(lambda:read()[field]==old-1,'decrement persisted')
                    click(width-16,y);wait(lambda:read()[field]==old,'increment persisted')
                    s.key('Left');wait(lambda:read()[field]==old-1,'keyboard decrement')
                    s.key('Right');wait(lambda:read()[field]==old,'keyboard increment')
                    check(field+' mouse/keyboard reversible persistence',read()[field]==old)
                    if row==6:s.shot('preferences');idle('preferences')
                    s.key('Escape')
                pref(7);s.key('Return');search('uNdO');s.shot('undo-search')
                click(60,94);old=undo();s.key('Escape') # clearing search proves row click did not capture
                search('Undo');button(0);s.key('ctrl+v');s.shot('conflict')
                check('conflict refuses atomically',undo()==old)
                button(0);s.key('Return') # Cancel capture then Enter -> Change again
                s.key('ctrl+alt+q');wait(lambda:any(b['control'].get('LogicalKey',{}).get('Character')=='q' for b in undo()),'logical remap persisted')
                check('logical remap '+layout,len(undo())==1)
                button(1);wait(lambda:not undo(),'mouse unassign')
                button(2);wait(lambda:undo()==old,'per-action defaults')
                check('Unassign and Reset action',undo()==old)
                s.key('Escape');search('ctrl+z');s.shot('shortcut-search');s.key('Escape');search('Pointer');s.shot('pointer-search')
                s.key('Escape');before_reset=read()['keymap'];button(2,1);s.shot('global-confirm');s.key('Escape')
                check('global reset cancel preserves mappings',read()['keymap']==before_reset)
                idle('keymap');s.key('Escape');idle('closed')
                # Reopen verifies persistent values and real in-panel import/export.
                if theme=='NeutralGray' and scale==1:
                    pref(7);s.key('Return');export=work/'export.json';button(1,1);pick(export,False)
                    check('in-panel canonical export',json.loads(export.read_text())['keymap']==read()['keymap'])
                    search('Undo');button(1);wait(lambda:not undo(),'unassign before import');button(0,1);pick(export,True)
                    wait(lambda:undo()==old,'in-panel import restored validated map');check('canonical import',undo()==old)
                    s.key('Escape');s.key('Escape')
                s.close()
                data=json.loads(s.report.read_text())
                for row in rows:
                    if row['case']==work.name:
                        row['idle_frames']=len([f for f in data['frames'] if row['begin_ms']-100<=f['elapsed_ms']<=row['end_ms']+100])
                        (root/'idle-raw.json').write_text(json.dumps(rows,indent=2))
                        check(row['state']+' zero settled redraw/IO/threads',row['idle_frames']==0 and not any(row['io'].values()) and row['thread_delta']==0)
                check('profile retained sizes',read()['handle_size']==initial['handle_size'] and read()['hit_radius']==initial['hit_radius'])
            finally:
                if s.process.poll() is None:s.kill()
            (root/'receipt.json').write_text(json.dumps({'binary_sha256':digest(a.binary),'idle':rows,'cases_completed':len({r['case'] for r in rows})},indent=2))
            print(work.name,len(checks),'checks',flush=True)
    command('setxkbmap','-layout','us')

if __name__=='__main__':main()
