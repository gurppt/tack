#!/usr/bin/env python3
"""Two owned native clients observe protocol-3 Frame/compound edits on loopback."""
import argparse,json,os,re,socket,subprocess,time,uuid
from pathlib import Path
from run_local_production import Session,wait
from run_phase2a_native import send_message,read_message,snapshot,native_revision
from run_native_annotation_checks import metadata_objects
from run_image_interaction import digest

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('binary','server','board','output'):p.add_argument('--'+name,type=Path,required=True)
    a=p.parse_args();root=a.output.resolve();root.mkdir(parents=True,exist_ok=False);binary=a.binary.resolve();server_binary=a.server.resolve();board_path=a.board.resolve()
    xroot=Path(__file__).resolve().parents[1]/'target/native/tools/xvfb-root'
    os.environ.update(DISPLAY=':112',TACK_NATIVE_NO_WM='1',WINIT_X11_SCALE_FACTOR='1',LP_NUM_THREADS='2',LD_LIBRARY_PATH=str(xroot/'usr/lib/x86_64-linux-gnu'),VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',WGPU_BACKEND='vulkan')
    xlog=(root/'xvfb.log').open('w');x=subprocess.Popen([str(xroot/'usr/bin/Xvfb'),':112','-screen','0','1280x1024x24','-nolisten','tcp','-noreset'],stdout=xlog,stderr=subprocess.STDOUT)
    serverlog=(root/'server.log').open('w');server=None;clients=[];peer=None;checks=[];revision=0
    def record(name,ok,detail=None):
        checks.append(dict(name=name,observed=bool(ok),detail=detail));(root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if not ok:raise AssertionError(name)
    try:
        wait(lambda:subprocess.run(['xdpyinfo'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'owned X11')
        server=subprocess.Popen([str(server_binary),'--listen','127.0.0.1:0','--root',str(root/'server-data'),'--asset-quota','33554432'],stdout=serverlog,stderr=subprocess.STDOUT)
        def listening():
            if server.poll() is not None:raise RuntimeError('owned server exited')
            m=re.search(r'listen=(\S+)',(root/'server.log').read_text());return m.group(1) if m else None
        address=wait(listening,'owned loopback server');subprocess.run([str(binary),'publish',str(board_path),address],capture_output=True,check=True,timeout=25)
        board=json.loads(subprocess.check_output([str(binary),'inspect',str(board_path)]))['document_id']
        initial=snapshot(address,board)['document'];before=metadata_objects(initial);frame=next(o for o in before['objects'] if o['kind']=='frame');child=next(o for o in before['objects'] if o['kind']==9)
        record('protocol3 snapshot contains schema7 compound Scribble and link',before['schema']==7 and [child['id'],frame['id']] in before['links'])
        for name in ['A','B']:
            client=Session(binary,root,name,['join',address,board],dict(os.environ,TACK_PROFILE_DIR=str(root/(name+'-profile')),TACK_TEST_WINDOW_SIZE='800x600'));clients.append(client);client.key('Escape')
        wait(lambda:all(native_revision(c)==0 for c in clients),'both native clients joined',seconds=20)
        host,port=address.rsplit(':',1);peer=socket.create_connection((host,int(port)),timeout=5);peer.settimeout(10)
        send_message(peer,{'type':'hello','board':board,'client':uuid.uuid4().hex,'revision':0});assert read_message(peer)['type']=='snapshot'
        def change(command=None,history=None):
            nonlocal revision
            message=dict(type=history or 'edit',operation=uuid.uuid4().hex,base=revision)
            if command:message.update(command=command,sources=[])
            send_message(peer,message);accepted=read_message(peer)
            for _ in range(16):
                if accepted['type']!='lease_snapshot':break
                accepted=read_message(peer)
            if accepted['type']!='accepted':raise AssertionError(accepted)
            revision+=1;wait(lambda:all(native_revision(c)==revision for c in clients),'native clients converge',seconds=15)
            result=snapshot(address,board)
            if result['revision']!=revision:raise AssertionError('authority revision')
            return result['document']
        fid=f"{frame['id']:032x}";cid=f"{child['id']:032x}"
        state=change({'kind':'set_frame_links','links':[[cid,None]]});decoded=metadata_objects(state)
        record('Unlink converges without moving child',decoded.get('links',[])==[] and next(o for o in decoded['objects'] if o['id']==child['id'])==child)
        state=change({'kind':'set_frame_links','links':[[cid,fid]]});record('Relink converges exactly',state==initial)
        transform=dict(center=[v+d for v,d in zip(frame['center'],[30,20])],size=frame['size'],rotation=0,flips=[False,False])
        moved=change({'kind':'set_transform','object':fid,'transform':transform});decoded=metadata_objects(moved);sc=next(o for o in decoded['objects'] if o['id']==child['id'])
        record('Frame translation converges to linked child in both clients',all(abs(sc['center'][j]-child['center'][j]-[30,20][j])<1e-8 for j in range(2)))
        record('Frame translation Undo converges exactly',change(history='undo')==initial)
        record('Frame translation Redo converges exactly',change(history='redo')==moved)
        style=dict(stroke=child['stroke'],fill=child['fill'],width=child['width'],opacity=child['opacity'])
        strokes=[dict(points=[[0,.25],[1,.25]],style=None),dict(points=[[0,.75],[1,.75]],style=dict(style,stroke=[255,0,0,255]))]
        replaced=change({'kind':'set_annotation','object':cid,'annotation':dict(kind='compound_scribble',strokes=strokes),'style':style});decoded=metadata_objects(replaced)
        record('compound replacement and per-stroke style converge',len(next(o for o in decoded['objects'] if o['id']==child['id'])['strokes'])==2)
        record('compound replacement Undo converges exactly',change(history='undo')==moved)
        record('compound replacement Redo converges exactly',change(history='redo')==replaced)
        for c in clients:c.shot('converged');c.close()
        clients.clear();peer.close();peer=None;server.terminate();server.wait(timeout=5);server=None
        (root/'receipt.json').write_text(json.dumps(dict(checks=len(checks),passed=all(c['observed'] for c in checks),binary_sha256=digest(binary),server_sha256=digest(server_binary),harness_sha256=digest(__file__),build_info=json.loads(subprocess.check_output([str(binary),'--build-info'])),scope='Two native Linux clients plus one protocol producer, same host software Vulkan/loopback; no physical two-machine LAN claim'),indent=2)+'\n')
    finally:
        if peer:peer.close()
        for c in clients:c.kill()
        if server:server.terminate();server.wait(timeout=5)
        serverlog.close();x.terminate();x.wait(timeout=5);xlog.close()
if __name__=='__main__':main()
