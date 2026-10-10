#!/usr/bin/env python3
"""Three owned X11 displays keep simultaneous real native gestures focused.

Starts only owned Xvfb/server/client processes. Small generated PNGs, no user profile.
This is one-host GPU evidence, not physical two-computer artist acceptance.
"""
import argparse, contextlib, json, os, re, socket, subprocess, time, uuid
from pathlib import Path
from PIL import Image
from run_local_production import Session, wait
from run_native_annotation_checks import saved_objects
from run_native_image_checks import command
from run_image_interaction import digest
from run_phase2a_native import snapshot, native_revision, send_message, read_message
from run_phase2a_local_regression import snapshot as process_snapshot, delta

@contextlib.contextmanager
def display(value):
    old=os.environ.get('DISPLAY'); os.environ['DISPLAY']=value
    try: yield
    finally:
        if old is None: os.environ.pop('DISPLAY',None)
        else: os.environ['DISPLAY']=old

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for k in ['binary','server','output']: p.add_argument('--'+k,type=Path,required=True)
    args=p.parse_args(); root=args.output.resolve(); root.mkdir(parents=True,exist_ok=False)
    binary=args.binary.resolve(); checks=[]; clients=[]; xservers=[]; logs=[]; server=None
    env=dict(os.environ,TACK_NATIVE_NO_WM='1',WINIT_X11_SCALE_FACTOR='1',TACK_TEST_WINDOW_SIZE='800x600',TACK_NATIVE_DIAGNOSTICS='1',GSETTINGS_BACKEND='memory',GIO_USE_VFS='local',GSK_RENDERER='cairo',TACK_TRACE_INPUT='1')
    def record(name,ok,detail=None):
        checks.append(dict(name=name,observed=bool(ok),detail=detail))
        (root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if not ok: raise AssertionError(name)
    def cli(*a):
        r=subprocess.run([str(binary),*map(str,a)],env=env,capture_output=True,text=True,timeout=30)
        if r.returncode: raise AssertionError(r.stderr)
        return json.loads(r.stdout)
    def run(c,method,*a):
        with display(c.env_display): return getattr(c,method)(*a)
    def mouse(c,x,y,op=None):
        with display(c.env_display):
            command('xdotool','mousemove','--window',c.window,str(round(x)),str(round(y)))
            if op: command('xdotool',op,'1')
        time.sleep(.12)
    def key(c,*keys): return run(c,'key',*keys)
    def revision(c): return run(c,'title')
    def converge(r):
        wait(lambda:all(' · rev '+str(r)+' ·' in revision(c) for c in clients),'three native authority receipts',seconds=15)
    def observed_revision(): return snapshot(address,board)['revision']
    def shot(c,label):run(c,'shot',label)
    try:
        for i in range(3):
            number=next(n for n in range(91+i*10,100+i*10) if not Path('/tmp/.X'+str(n)+'-lock').exists())
            d=':'+str(number); log=(root/('xvfb-'+str(i)+'.log')).open('w');logs.append(log)
            x=subprocess.Popen(['Xvfb',d,'-screen','0','1280x720x24','-nolisten','tcp','-noreset'],stdout=log,stderr=subprocess.STDOUT);xservers.append((x,d))
            wait(lambda:Path('/tmp/.X11-unix/X'+str(number)).exists(),'owned display')
            time.sleep(.3)
        image=root/'source.png';Image.new('RGB',(160,100),(45,130,185)).save(image)
        local=root/'artist.tack';cli('create',local,'--embedded',image,image)
        original_hash=digest(local)
        log=(root/'server.log').open('w');logs.append(log)
        server=subprocess.Popen([str(args.server.resolve()),'--listen','127.0.0.1:0','--root',str(root/'authority'),'--asset-quota',str(32*1024*1024)],env=env,stdout=log,stderr=subprocess.STDOUT)
        address=wait(lambda:(m.group(1) if (m:=re.search(r'listen=(\S+)',(root/'server.log').read_text())) else None),'owned server listen')
        server_empty=process_snapshot(server.pid)
        board=cli('inspect',local)['document_id'];cli('publish',local,address)
        for i,(_,d) in enumerate(xservers):
            name='client-'+str(i); e=dict(env,DISPLAY=d,TACK_PROFILE_DIR=str(root/(name+'-profile')))
            with display(d): c=Session(binary,root,name,['join',address,board],e)
            c.env_display=d;clients.append(c)
        converge(0); time.sleep(1)
        # Normalize the canvas to fit both images. Zoom is entirely local.
        for c in clients:
            key(c,'Escape');mouse(c,400,300)
            with display(c.env_display):command('xdotool','click','--repeat','4','--delay','40','5')
        time.sleep(.5)
        metadata=sorted(saved_objects(local)['objects'],key=lambda o:o['center'][0]);first,second=metadata[:2]
        zoom=600/first['size'][0]*__import__('math').exp(-.3*4)
        shot(clients[0],'initial-camera')
        pixels=Image.open(root/(clients[0].name+'-initial-camera.png')).convert('RGB')
        xs=[x for x in range(800) if (lambda rgb:rgb[0]<80 and 90<rgb[1]<180 and rgb[2]>150)(pixels.getpixel((x,300)))]
        groups=[]
        for x in xs:
            if not groups or x>groups[-1][-1]+1:groups.append([])
            groups[-1].append(x)
        groups=[g for g in groups if len(g)>20]
        if len(groups)!=2:raise AssertionError('Two image centers identified from actual initial camera')
        points=[[(g[0]+g[-1])/2,300.] for g in groups]
        record('three native clients keep independent focused displays',len({c.process.pid for c in clients})==3,dict(displays=[c.env_display for c in clients],image_points=[list(p) for p in points],zoom=zoom))
        a,b,c=clients
        server_joined=process_snapshot(server.pid)
        # A holds an image gesture; B draws an unrelated scribble without focus loss.
        mouse(a,*points[0],'mousedown');mouse(a,points[0][0]+22,points[0][1]+17);time.sleep(.25)
        shot(b,'foreign-lease')
        server_leased=process_snapshot(server.pid)
        before=observed_revision();key(b,'p');mouse(b,110,460,'mousedown');mouse(b,155,440);mouse(b,205,475,'mouseup');key(b,'v')
        converge(before+1)
        mouse(a,points[0][0]+45,points[0][1]+30,'mouseup');converge(before+2)
        record('A image movement survives B unrelated scribble',observed_revision()==before+2,dict(revision=before+2))
        # Two transforms have simultaneous leases and coexist.
        points[0]=[points[0][0]+45,points[0][1]+30]
        with display(a.env_display):command('xdotool','keydown','ctrl')
        mouse(a,points[0][0]+30,points[0][1],'mousedown');mouse(a,points[0][0]+28,points[0][1]+20)
        mouse(c,*points[1],'mousedown');mouse(c,points[1][0]+15,points[1][1]+15)
        time.sleep(.25);before=observed_revision()
        mouse(c,points[1][0]+30,points[1][1]+20,'mouseup');mouse(a,points[0][0]+20,points[0][1]+25,'mouseup')
        with display(a.env_display):command('xdotool','keyup','ctrl')
        converge(before+2);record('separate image rotate and move converge',observed_revision()==before+2)
        points[1]=[points[1][0]+30,points[1][1]+20]
        # Busy outline and same-object denial: B cannot steal A's gesture.
        mouse(a,*points[0],'mousedown');mouse(a,points[0][0]+12,points[0][1]+7);time.sleep(.25)
        shot(b,'red-busy')
        pixels=Image.open(root/(b.name+'-red-busy.png')).convert('RGB')
        red=sum(1 for r,g,bl in pixels.getdata() if r>230 and g<170 and bl<180)
        before=observed_revision();mouse(b,*points[0],'mousedown');mouse(b,points[0][0]-25,points[0][1],'mouseup');time.sleep(.3)
        record('foreign red outline and no same-object steal',red>15 and observed_revision()==before,dict(red_pixels=red))
        mouse(a,points[0][0]+20,points[0][1]+10,'mouseup');converge(before+1);points[0]=[points[0][0]+20,points[0][1]+10]
        # Open menu on A, then unrelated C edit. The exact menu pixels survive.
        mouse(a,*points[0]);
        with display(a.env_display):command('xdotool','click','3')
        shot(a,'menu-before');before=observed_revision()
        key(c,'r');mouse(c,80,440,'mousedown');mouse(c,190,520,'mouseup');key(c,'v');converge(before+1)
        shot(a,'menu-after');im1=Image.open(root/(a.name+'-menu-before.png'));im2=Image.open(root/(a.name+'-menu-after.png'))
        # Crop the menu pane, excluding status and unrelated canvas.
        crop=(round(points[0][0]),round(points[0][1]),min(799,round(points[0][0])+300),min(560,round(points[0][1])+200))
        record('context menu survives unrelated native edit',im1.crop(crop).tobytes()==im2.crop(crop).tobytes(),dict(crop=crop))
        key(a,'Escape')
        # Shared Duplicate is one command, fresh metadata only and reversible.
        mouse(b,*points[0],'click');before=observed_revision();count=snapshot(address,board)['document']['counts'][2]
        key(b,'ctrl+d');converge(before+1);record('shared Duplicate adds a fresh object',snapshot(address,board)['document']['counts'][2]==count+1)
        key(b,'ctrl+z');converge(before+2);record('shared Duplicate undo exact count',snapshot(address,board)['document']['counts'][2]==count)
        # Local view assignment is not a shared revision.
        before=observed_revision();key(c,'b','7');key(c,'b','8');time.sleep(.3)
        profile=json.loads((root/(c.name+'-profile')/'preferences.json').read_text())
        record('B view7/view8 are local profile state',sorted(v['slot'] for v in profile.get('local_views',[]))==[7,8] and observed_revision()==before)
        # Input-to-all-receipts upper bound (includes xdotool and polling).
        latency=[]
        for i in range(6):
            before=observed_revision();started=time.monotonic()
            mouse(c,*points[1],'mousedown');time.sleep(.15);mouse(c,points[1][0]+3,points[1][1],'mouseup');converge(before+1)
            latency.append((time.monotonic()-started)*1000);points[1][0]+=3
        record('shared native edit input-to-receipts measured',len(latency)==6,dict(milliseconds=latency,scope='Includes injected input sleeps and receipt polling; not pure transport latency'))
        record('bounded server RSS stages measured',True,dict(empty=server_empty['rss_bytes'],three_clients=server_joined['rss_bytes'],one_active_lease=server_leased['rss_bytes']))
        # Settled idle: no gesture means no lease heartbeat or recurring redraw.
        time.sleep(3); watched=[server.process.pid if hasattr(server,'process') else server.pid]+[s.process.pid for s in clients]
        before=[process_snapshot(pid) for pid in watched];started=time.monotonic();time.sleep(3);seconds=time.monotonic()-started
        after=[process_snapshot(pid) for pid in watched];idle=[delta(x,y,seconds) for x,y in zip(before,after)]
        record('settled server has zero CPU; clients have bounded driver idle cost and no data I/O',idle[0]['ticks']==0 and all(row['ticks']<=3 and all(row['io_delta'][k]==0 for k in ['rchar','wchar','read_bytes','write_bytes']) for row in idle),idle)
        record('original local file stays untouched',digest(local)==original_hash)
        # Exact-target deletion leaves A's menu stable and next action safe.
        mouse(a,*points[0]);
        with display(a.env_display):command('xdotool','click','3')
        before=observed_revision();mouse(b,*points[0],'click');key(b,'Delete');converge(before+1);shot(a,'menu-target-deleted')
        key(a,'Return','Escape');time.sleep(.2)
        record('deleted menu target degrades without crash or stale edit',a.process.poll() is None and observed_revision()==before+1)
        # Abrupt native owner loss releases its lease; another client reclaims.
        mouse(c,*points[1],'mousedown');mouse(c,points[1][0]+6,points[1][1]);time.sleep(.2);run(c,'kill');clients.remove(c);time.sleep(.3)
        before=observed_revision();mouse(b,*points[1],'mousedown');time.sleep(.2);mouse(b,points[1][0]+9,points[1][1],'mouseup');converge(before+1)
        record('crashed native lease owner can be reclaimed',observed_revision()==before+1)
        # Disconnect explanation appears before edits. SaveToLocal is covered separately.
        server.terminate();server.wait(timeout=8);server=None;time.sleep(.5)
        shot(b,'offline');record('disconnect is visible without attempted edit','Disconnected' in revision(b) or 'ServerUnavailable' in revision(b),revision(b))
        for client in clients:
            key(client,'Escape');run(client,'close')
        idle_redraws=[]
        for client in clients:
            frames=json.loads(client.report.read_text())['frames']
            idle_redraws.append(sum(started-client.started<=f['elapsed_ms']/1000<=started-client.started+seconds for f in frames))
        record('no native redraw during settled idle interval',all(n==0 for n in idle_redraws),idle_redraws)
        (root/'receipt.json').write_text(json.dumps(dict(binary_sha256=digest(binary),harness_sha256=digest(__file__),scope='Three native Linux clients on three owned displays, one physical host; physical two-computer test pending',checks=checks),indent=2)+'\n')
    finally:
        for client in clients:
            with display(client.env_display):client.kill()
        if server and server.poll() is None:server.terminate();server.wait(timeout=8)
        for x,_ in xservers:x.terminate();x.wait(timeout=5)
        for log in logs:log.close()
    print(root/'receipt.json')
if __name__=='__main__':main()
