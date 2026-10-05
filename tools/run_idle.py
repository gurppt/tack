#!/usr/bin/env python3
"""Untimed ordinary windows: externally observe /proc idle without application timers."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time
from run_image_interaction import digest
from run_native_image_checks import command


def observe(pid):
    tasks = {}
    for p in Path(f'/proc/{pid}/task').iterdir():
        raw = (p/'stat').read_text(); fields = raw[raw.rfind(')')+2:].split()
        status = (p/'status').read_text().splitlines()
        switches = {s.split(':')[0]:int(s.split()[1]) for s in status if s.startswith(('voluntary_ctxt_switches:', 'nonvoluntary_ctxt_switches:'))}
        tasks[p.name] = {'name':(p/'comm').read_text().strip(), 'ticks':int(fields[11])+int(fields[12]), **switches}
    status = Path(f'/proc/{pid}/status').read_text().splitlines()
    rss = next(int(s.split()[1])*1024 for s in status if s.startswith('VmRSS:'))
    io = {s.split(':')[0]:int(s.split()[1]) for s in Path(f'/proc/{pid}/io').read_text().splitlines()}
    sockets = []
    for fd in Path(f'/proc/{pid}/fd').iterdir():
        try:
            target=os.readlink(fd)
            if target.startswith('socket:'):sockets.append(target)
        except FileNotFoundError:pass
    return {'tasks':tasks,'rss_bytes':rss,'io':io,'socket_count':len(sockets)}


def close_owned_window(window):
    """WM_DELETE_WINDOW for only the harness-owned client, without XDestroyWindow."""
    import ctypes as c
    class Data(c.Union):
        _fields_=[('l',c.c_long*5),('b',c.c_char*20)]
    class Client(c.Structure):
        _fields_=[('type',c.c_int),('serial',c.c_ulong),('send_event',c.c_int),('display',c.c_void_p),('window',c.c_ulong),('message_type',c.c_ulong),('format',c.c_int),('data',Data)]
    class Event(c.Union):
        _fields_=[('client',Client),('padding',c.c_long*24)]
    x=c.CDLL('libX11.so.6');x.XOpenDisplay.restype=c.c_void_p;x.XOpenDisplay.argtypes=[c.c_char_p]
    x.XInternAtom.restype=c.c_ulong;x.XInternAtom.argtypes=[c.c_void_p,c.c_char_p,c.c_int]
    x.XSendEvent.argtypes=[c.c_void_p,c.c_ulong,c.c_int,c.c_long,c.POINTER(Event)]
    x.XFlush.argtypes=[c.c_void_p];x.XCloseDisplay.argtypes=[c.c_void_p]
    display=x.XOpenDisplay(None)
    if not display:raise RuntimeError('isolated display unavailable')
    try:
        event=Event();event.client.type=33;event.client.display=display;event.client.window=int(window);event.client.message_type=x.XInternAtom(display,b'WM_PROTOCOLS',0);event.client.format=32;event.client.data.l[0]=x.XInternAtom(display,b'WM_DELETE_WINDOW',0)
        if not x.XSendEvent(display,int(window),0,0,c.byref(event)):raise RuntimeError('owned close event failed')
        x.XFlush(display)
    finally:x.XCloseDisplay(display)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=Path('target/release/tack-app'))
    parser.add_argument('--board',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--seconds',type=float,default=10)
    parser.add_argument('--no-wm',action='store_true',help='Isolated X11 display: direct focus and scoped WM_DELETE, no desktop shortcuts')
    parser.add_argument('--edit-note',action='store_true',help='Observe transient new note editing without a caret timer')
    parser.add_argument('--context-menu',choices=('none','canvas','application','closed'),default='none',help='Mission 1H: observe an open static popup or the settled state after closing it')
    parser.add_argument('--baseline',action='store_true',help='One hidden-grid observation for older binaries without spatial telemetry')
    args=parser.parse_args()
    if args.context_menu!='none' and (not args.no_wm or os.environ.get('DISPLAY') in (None,':0',':0.0')):parser.error('context-menu checks require an explicitly owned isolated X11 display and --no-wm')
    if args.context_menu!='none' and args.edit_note:parser.error('note draft and menu probes are separate observations')
    if not 5<=args.seconds<=30:parser.error('interval 5..30 seconds')
    root=args.output.resolve();root.mkdir(parents=True,exist_ok=False);binary=args.binary.resolve();board=args.board.resolve();before_hash=digest(board);rows=[]
    for visible in ((False,) if args.baseline else (False,True)):
        name='visible' if visible else 'hidden';report=root/f'{name}.json';started=time.monotonic()
        with (root/f'{name}.log').open('w') as log:
            process=subprocess.Popen([str(binary),'open',str(board),'--output',str(report)],stdout=log,stderr=subprocess.STDOUT)
            try:
                window=None
                while time.monotonic()-started<6:
                    try:
                        windows=command('xdotool','search','--onlyvisible','--pid',process.pid).splitlines()
                        if windows:window=windows[-1];break
                    except subprocess.CalledProcessError:pass
                    time.sleep(.05)
                if window is None:raise RuntimeError('owned window not mapped')
                # Mapping precedes asynchronous document/GPU initialization.
                # Wait for the authoring title before sending tool/grid keys.
                while time.monotonic()-started<10:
                    title=command('xdotool','getwindowname',window)
                    if 'selected' in title:break
                    time.sleep(.05)
                else:raise RuntimeError('owned authoring window not ready')
                command('xdotool','windowfocus' if args.no_wm else 'windowactivate','--sync',window)
                if visible:command('xdotool','key','g')
                if args.edit_note:
                    command('xdotool','key','t');command('xdotool','mousemove','--window',window,30,30);command('xdotool','click','1')
                if args.context_menu!='none':
                    if args.context_menu=='application':command('xdotool','key','F10')
                    else:
                        command('xdotool','mousemove','--window',window,250,180);command('xdotool','click','3')
                    if args.context_menu=='closed':command('xdotool','key','Escape')
                time.sleep(3)
                observed_title=command('xdotool','getwindowname',window).strip()
                if args.edit_note and 'Ctrl+Enter confirm' not in observed_title:
                    raise AssertionError('note editor did not become active')
                begin=time.monotonic();initial=observe(process.pid);time.sleep(args.seconds);final=observe(process.pid);end=time.monotonic()
                if args.edit_note:command('xdotool','key','Escape')
                if args.context_menu in ('canvas','application'):command('xdotool','key','Escape')
                if args.no_wm:close_owned_window(window)
                else:command('xdotool','key','alt+F4')
                process.wait(timeout=5)
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:process.wait(timeout=2)
                    except subprocess.TimeoutExpired:process.kill();process.wait()
        if process.returncode:raise RuntimeError(f'idle exit {process.returncode}')
        data=json.loads(report.read_text());frames=[f for f in data['frames'] if (begin-started)*1000<=f['elapsed_ms']<=(end-started)*1000]
        if data.get('spatial',{}).get('grid',visible)!=visible:
            raise AssertionError('requested grid state not observed')
        delta_tasks=[]
        for tid,old in initial['tasks'].items():
            new=final['tasks'].get(tid)
            if new:delta_tasks.append({'name':old['name'],'cpu_ticks':new['ticks']-old['ticks'],'voluntary_switches':new['voluntary_ctxt_switches']-old['voluntary_ctxt_switches'],'involuntary_switches':new['nonvoluntary_ctxt_switches']-old['nonvoluntary_ctxt_switches']})
        ticks=sum(t['cpu_ticks'] for t in delta_tasks);duration=end-begin
        row={'grid':name,'interval_seconds':duration,'cpu_ticks':ticks,'cpu_percent_of_one_core':ticks/os.sysconf('SC_CLK_TCK')/duration*100,'rss_initial_bytes':initial['rss_bytes'],'rss_final_bytes':final['rss_bytes'],'idle_redraws':len(frames),'idle_gpu_submissions':len(frames),'total_redraws':data.get('redraw_count',len(data['frames'])),'total_waits':data.get('wait_count'),'threads':delta_tasks,'new_threads':sorted(set(final['tasks'])-set(initial['tasks'])),'departed_threads':sorted(set(initial['tasks'])-set(final['tasks'])),'io_delta':{k:final['io'][k]-initial['io'][k] for k in initial['io']},'socket_count':final['socket_count'],'source_bytes':data['source_bytes_before_detail'],'report_sha256':digest(report)}
        if frames or data['source_bytes_before_detail']!=0:raise AssertionError('idle redraw/source-I/O gate')
        row['observed_title']=observed_title
        rows.append(row);(root/'observations.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,'idle redraws',len(frames),'ticks',ticks,flush=True)
    if digest(board)!=before_hash:raise AssertionError('idle input board changed')
    summary={'context_menu':args.context_menu,'baseline':args.baseline,'note_editing':args.edit_note,'display':os.environ.get('DISPLAY'),'isolated_no_wm':args.no_wm,'binary_sha256':digest(binary),'harness_sha256':digest(__file__),'board_sha256':before_hash,'application_timer':'none; ordinary open has no --seconds/--interaction/tour','network':'Tack has no network client; socket_count includes native X11/driver IPC, not a network-byte counter','wakeups':'per-thread voluntary/involuntary context switches; total event-loop waits include startup/shutdown, not exact idle wakeups','gpu':'redraw submissions observed; nvidia-smi global utilization is not per-process attribution','runs':rows}
    (root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')


if __name__=='__main__':main()
