#!/usr/bin/env python3
"""Bounded X11 authoring checks, owned generated images/board and window captures."""
import argparse
import json
from pathlib import Path
import struct
import subprocess
import time
from PIL import Image
from run_native_image_checks import command
from run_image_interaction import digest


def saved_objects(path):
    return decoded_objects(path.read_bytes())


def metadata_objects(record):
    header=bytearray(80)
    struct.pack_into('<I',header,12,record['schema'])
    struct.pack_into('<III',header,48,*record['counts'])
    return decoded_objects(header+bytes.fromhex(record['metadata']))


def decoded_objects(data):
    schema=struct.unpack_from('<I',data,12)[0]
    sources,assets,objects=struct.unpack_from('<III',data,48);p=96;records=[]
    for _ in range(sources):n=struct.unpack_from('<I',data,p)[0];p+=4+n
    p+=assets*42
    for _ in range(objects):
        version,kind=struct.unpack_from('<HH',data,p);identity=int.from_bytes(data[p+4:p+20],'little');p+=20
        if version!=1:raise ValueError('version')
        if kind==1:
            center=struct.unpack_from('<dd',data,p+16);size=struct.unpack_from('<dd',data,p+32);p+=99
            records.append({'id':identity,'kind':'image','center':center,'size':size,'rotation':struct.unpack_from('<d',data,p-51)[0]})
        elif kind==2:
            center=struct.unpack_from('<dd',data,p);size=struct.unpack_from('<dd',data,p+16);n=struct.unpack_from('<H',data,p+32)[0];name=data[p+34:p+34+n].decode();p+=34+n
            records.append({'id':identity,'kind':'frame','center':center,'size':size,'name':name})
        elif 3<=kind<=9:
            n=struct.unpack_from('<I',data,p)[0];p+=4;payload=data[p:p+n];p+=n
            r={'id':identity,'kind':kind,'center':struct.unpack_from('<dd',payload,0),'size':struct.unpack_from('<dd',payload,16),'rotation':struct.unpack_from('<d',payload,32)[0],'stroke':list(payload[42:46]),'filled':payload[46],'fill':list(payload[47:51]) if payload[46] else None,'width':struct.unpack_from('<d',payload,51)[0],'opacity':struct.unpack_from('<d',payload,59)[0]}
            if kind==3:r.update(font_size=struct.unpack_from('<d',payload,67)[0],alignment=payload[75],text=payload[80:].decode())
            if kind==8:r['points']=struct.unpack_from('<I',payload,67)[0]
            if kind==9:
                count=struct.unpack_from('<I',payload,67)[0];q=71;strokes=[]
                for _ in range(count):
                    styled=payload[q];q+=1;style=None
                    if styled:
                        style={'stroke':list(payload[q:q+4]),'width':struct.unpack_from('<d',payload,q+9)[0],'opacity':struct.unpack_from('<d',payload,q+17)[0]};q+=25
                    points=struct.unpack_from('<I',payload,q)[0];q+=4+16*points
                    strokes.append({'points':points,'style':style})
                if q!=len(payload):raise ValueError('compound stroke length')
                r.update(strokes=strokes,points=sum(stroke['points'] for stroke in strokes))
            records.append(r)
        else:raise ValueError('kind')
    p+=objects*16;groups=struct.unpack_from('<I',data,p)[0] if schema>=2 else 0
    if schema>=2:
        p+=4
        for _ in range(groups):
            count=struct.unpack_from('<I',data,p+18)[0];p+=22+16*count
    if schema>=4:
        count=struct.unpack_from('<I',data,p)[0];p+=4
        for _ in range(count):
            n=struct.unpack_from('<H',data,p+42)[0];p+=44+n
    if schema>=5:
        count=struct.unpack_from('<I',data,p)[0];p+=4+20*count
    links=[]
    if schema>=7:
        count=struct.unpack_from('<I',data,p)[0];p+=4
        for _ in range(count):
            links.append([int.from_bytes(data[p:p+16],'little'),int.from_bytes(data[p+16:p+32],'little')]);p+=32
    result={'schema':schema,'objects':records,'groups':groups}
    if schema>=7:result['links']=links
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--binary',type=Path,default=Path('target/release/tack-app'));parser.add_argument('--output',type=Path,required=True);parser.add_argument('--source-actions',action='store_true');args=parser.parse_args()
    root=args.output.resolve();root.mkdir(parents=True,exist_ok=False);binary=args.binary.resolve();board=root/'board.tack'
    Image.new('RGB',(320,240),(190,70,40)).save(root/'a.png');Image.new('RGB',(320,240),(40,100,190)).save(root/'b.png')
    subprocess.run([str(binary),'create',str(board),'--linked',str(root/'a.png'),str(root/'b.png')],stdout=(root/'create.log').open('w'),stderr=subprocess.STDOUT,check=True,timeout=20)
    started=time.monotonic();checks=[]
    with (root/'native.log').open('w') as log:
        process=subprocess.Popen([str(binary),'open',str(board),'--output',str(root/'native.json')],stdout=log,stderr=subprocess.STDOUT)
        try:
            window=None
            while time.monotonic()-started<8:
                try:
                    windows=command('xdotool','search','--onlyvisible','--pid',process.pid).splitlines()
                    if windows:window=windows[-1];break
                except subprocess.CalledProcessError:pass
                time.sleep(.05)
            if window is None:raise RuntimeError('owned window unavailable')
            command('xdotool','windowactivate','--sync',window);time.sleep(.5);command('xdotool','keyup','ctrl','alt','shift','super','x')
            def title():return command('xdotool','getwindowname',window)
            def key(k):command('xdotool','key',k);time.sleep(.1)
            def pointer(x,y):command('xdotool','mousemove','--window',window,round(x),round(y));time.sleep(.035)
            def record(name,ok):
                checks.append({'name':name,'observed':bool(ok),'title':title() if process.poll() is None else 'closed'});(root/'checks.json').write_text(json.dumps(checks,indent=2))
                if not ok:raise AssertionError(name)
                if time.monotonic()-started>100:raise RuntimeError('native deadline')
            def capture(name):
                time.sleep(.15);p=root/f'{name}.png';subprocess.run(['import','-window',window,str(p)],check=True,timeout=4);return Image.open(p).convert('RGB')
            def drag(start,end,cancel=False):
                pointer(*start);command('xdotool','mousedown','1')
                for i in range(1,7):pointer(start[0]+(end[0]-start[0])*i/6,start[1]+(end[1]-start[1])*i/6)
                if cancel:key('Escape')
                command('xdotool','mouseup','1');time.sleep(.12)
            def save():
                key('ctrl+s')
                for _ in range(60):
                    if 'saved' in title() and 'saving' not in title():return saved_objects(board)
                    time.sleep(.04)
                raise AssertionError('save deadline')
            initial=capture('initial');key('t');drag((80,80),(300,205));command('xdotool','type','--clearmodifiers','Review note');key('Return');command('xdotool','type','--clearmodifiers','Second line');key('ctrl+Return');text=save();notes=[o for o in text['objects'] if o['kind']==3];record('native text creation, multiline and atomic commit',len(notes)==1 and notes[0]['text']=='Review note\nSecond line')
            key('F2');command('xdotool','type','--clearmodifiers','Cancelled');key('Escape');record('native edit Escape preserves note',save()==text)
            key('F2');key('ctrl+a');command('xdotool','type','--clearmodifiers','Edited note');key('ctrl+Return');edited=save();record('native note edit persists',next(o for o in edited['objects'] if o['kind']==3)['text']=='Edited note')
            key('ctrl+z');record('note undo exact',save()==text);key('ctrl+y');record('note redo exact',save()==edited)
            for k,kind,start,end in [('r',4,(80,235),(275,310)),('l',6,(80,450),(275,470)),('a',7,(80,505),(275,530))]:
                key(k);drag(start,end);record(f'native create kind {kind}',any(o['kind']==kind for o in save()['objects']))
            key('p');before=save();drag((80,580),(280,635),cancel=True);record('scribble Escape discards preview',save()==before)
            pointer(80,580);command('xdotool','mousedown','1')
            for i in range(30):pointer(80+i*6,580+(i%4)*8)
            capture('stroke-live');command('xdotool','mouseup','1');stroke=save();record('native scribble commits bounded vector',any(o['kind']==8 and 2<=o['points']<=4096 for o in stroke['objects']))
            key('v');pointer(80,235);command('xdotool','click','1');key('c');key('f');key('bracketright');key('shift+bracketleft');styled=save();rect=next(o for o in styled['objects'] if o['kind']==4);record('semantic color fill width opacity styling',rect['stroke']!=[255,198,82,255] and rect['filled']==1 and rect['width']>3 and rect['opacity']<1)
            drag((120,260),(150,280));moved=save();record('annotation move changes transform',next(o for o in moved['objects'] if o['kind']==4)['center']!=rect['center']);key('ctrl+z');record('annotation move undo exact',save()==styled)
            drag((275,310),(305,335));resized=save();record('native annotation resize',next(o for o in resized['objects'] if o['kind']==4)['size']!=rect['size']);key('ctrl+z');record('resize undo exact',save()==styled)
            import os
            dpi=max(1, int(float(os.environ.get('WINIT_X11_SCALE_FACTOR','1')) + .5))
            drag((177.5,235-26*dpi),(235,220));rotated=save();record('native annotation rotate',abs(next(o for o in rotated['objects'] if o['kind']==4)['rotation'])>0.01);key('ctrl+z');record('rotate undo exact',save()==styled)
            pointer(640,360);command('xdotool','click','1');key('ctrl+shift+f');framed=save();record('frame coexists with annotations',any(o['kind']=='frame' for o in framed['objects']))
            key('ctrl+a');key('ctrl+g');record('mixed grouping deferred with explicit error','image' in title().lower() and 'group' in title().lower());key('Escape')
            drag((30,25),(990,680));record('mixed marquee selects images and annotations','selected' in title() and '0 selected' not in title())
            pointer(20,700);command('xdotool','click','1');pointer(640,360);command('xdotool','click','1')
            if args.source_actions:
                for name,k in [('Open','ctrl+shift+o'),('Reveal','ctrl+alt+o'),('Copy','ctrl+shift+c')]:
                    key(k)
                    for _ in range(100):
                        if ' · source action' not in title():break
                        time.sleep(.05)
                    record(f'native source {name} completed','helper' not in title() and 'failed' not in title() and 'require' not in title())
                    command('xdotool','windowactivate','--sync',window)
                clip=command('xclip','-selection','clipboard','-o').strip();record('clipboard contains actual canonical descriptor',clip==str((root/'a.png').resolve()))
                time.sleep(.6);command('xdotool','windowactivate','--sync',window)
                (root/'a.png').rename(root/'a.png.hidden')
                try:
                    key('ctrl+shift+o');time.sleep(.2);record('native missing source reports safe error','missing or unavailable' in title())
                finally:(root/'a.png.hidden').rename(root/'a.png')
            time.sleep(.6);command('xdotool','windowactivate','--sync',window);command('xdotool','windowraise',window);time.sleep(.2)
            key('v');pointer(640,360);command('xdotool','click','5');command('xdotool','click','5');low=capture('low-zoom');command('xdotool','click','4');command('xdotool','click','4');command('xdotool','click','4');high=capture('high-zoom');record('annotations remain visible across native zoom',low.tobytes()!=high.tobytes());command('xdotool','click','5')
            pointer(170,110);command('xdotool','click','1');key('ctrl+shift+period');key('ctrl+shift+e');capture('final')
            final=save();record('schema3 retains all five kinds with frame/images',final['schema']==3 and all(any(o['kind']==k for o in final['objects']) for k in (3,4,6,7,8)))
            pointer(20,700);command('xdotool','click','1');pointer(80,450);command('xdotool','click','1');key('Delete');deleted=save();record('native annotation delete',len(deleted['objects'])==len(final['objects'])-1);key('ctrl+z');record('native delete undo exact',save()==final)
            key('alt+F4');process.wait(timeout=6)
        finally:
            command('xdotool','keyup','ctrl','alt','shift','super','x')
            if process.poll() is None:process.terminate();process.wait(timeout=3)
    if process.returncode:raise RuntimeError(f'native exited {process.returncode}')
    original_hash=digest(board)
    subprocess.run([str(binary),'inspect',str(board)],stdout=(root/'reopen.json').open('w'),check=True,timeout=10)
    record('save/reopen keeps authored authority exact',saved_objects(board)==final and digest(board)==original_hash)
    (root/'provenance.json').write_text(json.dumps({'binary_sha256':digest(binary),'harness_sha256':digest(__file__),'board_sha256':original_hash,'checks':len(checks),'dpi_env':__import__('os').environ.get('WINIT_X11_SCALE_FACTOR'),'native':'X11 automated physical mouse/key input and owned captures; no artist/tablet feel claim'},indent=2)+'\n')


if __name__=='__main__':main()
