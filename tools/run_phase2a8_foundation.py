#!/usr/bin/env python3
"""Owned X11/software Vulkan foundation UI checks; never use the artist profile."""
import argparse, ctypes, ctypes.util, hashlib, json, os, subprocess, time
from pathlib import Path
from PIL import Image
from run_local_production import Session, wait
from run_native_image_checks import command
from run_phase2a_local_regression import snapshot, delta

class CursorImage(ctypes.Structure):
    _fields_ = [('x', ctypes.c_short), ('y', ctypes.c_short),
                ('width', ctypes.c_ushort), ('height', ctypes.c_ushort),
                ('xhot', ctypes.c_ushort), ('yhot', ctypes.c_ushort),
                ('serial', ctypes.c_ulong), ('pixels', ctypes.POINTER(ctypes.c_ulong)),
                ('atom', ctypes.c_ulong), ('name', ctypes.c_char_p)]

class Cursor:
    def __init__(self):
        self.x11 = ctypes.CDLL(ctypes.util.find_library('X11'))
        self.fix = ctypes.CDLL(ctypes.util.find_library('Xfixes'))
        self.x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
        self.x11.XOpenDisplay.restype = ctypes.c_void_p
        self.x11.XFree.argtypes = [ctypes.c_void_p]
        self.x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
        self.fix.XFixesGetCursorImage.argtypes = [ctypes.c_void_p]
        self.fix.XFixesGetCursorImage.restype = ctypes.POINTER(CursorImage)
        self.display = self.x11.XOpenDisplay(os.environ['DISPLAY'].encode())
        if not self.display: raise RuntimeError('Owned cursor display unavailable')
    def pixels(self):
        result = self.fix.XFixesGetCursorImage(self.display)
        if not result: raise RuntimeError('Cursor readback unavailable')
        try:
            c = result.contents
            if c.width*c.height > 4096: raise RuntimeError('Unexpected cursor extent')
            return dict(size=[c.width,c.height], hotspot=[c.xhot,c.yhot],
                        pixels=[c.pixels[i]&0xffffffff for i in range(c.width*c.height)])
        finally: self.x11.XFree(result)
    def close(self): self.x11.XCloseDisplay(self.display)

def expected(path, hotspot):
    img = Image.open(path).convert('RGBA')
    return dict(size=list(img.size),hotspot=hotspot,
                pixels=[(a<<24)|(r<<16)|(g<<8)|b for r,g,b,a in img.getdata()])

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True)
    p.add_argument('--profile',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args(); root=a.output.resolve(); root.mkdir(parents=True,exist_ok=False)
    source=Path(__file__).resolve().parents[1]; binary=a.binary.resolve()
    xroot=source/'target/native/tools/xvfb-root'
    os.environ.update(DISPLAY=':108', TACK_NATIVE_NO_WM='1', WINIT_X11_SCALE_FACTOR='1',
        LD_LIBRARY_PATH=str(xroot/'usr/lib/x86_64-linux-gnu'),
        VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json', WGPU_BACKEND='vulkan')
    xlog=(root/'xvfb.log').open('w')
    x=subprocess.Popen([str(xroot/'usr/bin/Xvfb'),':108','-screen','0','1280x1024x24','-nolisten','tcp','-noreset'],stdout=xlog,stderr=subprocess.STDOUT)
    session=None; cursor=None; checks=[]
    def record(name,ok,detail=None):
        checks.append(dict(name=name,observed=bool(ok),detail=detail))
        (root/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
        if not ok: raise AssertionError(name)
    def motion(x,y):
        command('xdotool','mousemove','--window',session.window,str(x),str(y)); time.sleep(.15)
    def pointer(): return cursor.pixels()==expected(binary.parent/'gfx/cursors/cursor_pointer.png',[1,1])
    try:
        wait(lambda:subprocess.run(['xdpyinfo'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0,'owned Xvfb')
        cursor=Cursor()
        image=root/'fixture.png'; Image.new('RGB',(160,100),(40,120,190)).save(image)
        board=root/'fixture.tack'; subprocess.run([str(binary),'create',str(board),'--embedded',str(image)],check=True,capture_output=True,timeout=20)
        settings=json.loads(a.profile.read_text()); settings.update(recent=[],local_views=[],update_channel='dev',status_bar=True)
        settings['toolbar'].update(scale=1,placement='Top',edge_position=0)
        remaps={'F1':'CheckForUpdates','F2':'SelectTool(Rectangle)','F3':'Preferences','F4':'About'}
        settings['keymap']=[b for b in settings['keymap'] if b['action'] not in remaps.values() and b['control'] not in [{'LogicalKey':{'Named':key}} for key in remaps]]
        for key,action in remaps.items(): settings['keymap'].append(dict(action=action,control={'LogicalKey':{'Named':key}},modifiers=0,modifier_match='Exact',trigger='Press'))
        profile=root/'profile'; profile.mkdir(); (profile/'preferences.json').write_text(json.dumps(settings))
        session=Session(binary,root,'foundation',['open',board],dict(os.environ,TACK_PROFILE_DIR=str(profile),TACK_TEST_WINDOW_SIZE='800x600'))
        time.sleep(1); session.key('Escape'); motion(700,400)
        before=snapshot(session.process.pid); time.sleep(2); after=snapshot(session.process.pid)
        idle=delta(before,after,2)
        record('idle has no TCP/UDP network socket',not any(s['kind'] in ['tcp','tcp6','udp','udp6'] for s in idle['socket_records']),idle)
        children=Path(f'/proc/{session.process.pid}/task/{session.process.pid}/children').read_text().split()
        record('idle has no updater child',not children,children)
        record('initial native cursor exactly matches installed pixel pointer',pointer())
        session.key('F2'); motion(701,401)
        tool=cursor.pixels()
        record('Rectangle canvas uses cached custom crosshair',tool==expected(binary.parent/'gfx/cursors/cursor_crosshair.png',[7,7]))
        motion(10,10); record('toolbar uses normal custom pointer',pointer())
        motion(702,402); record('canvas restores original tool cursor',cursor.pixels()==tool)
        session.key('F4'); motion(300,250); record('About uses pointer without changing tool',pointer()); session.shot('about')
        session.key('Escape'); motion(703,403); record('About dismissal restores active tool cursor',cursor.pixels()==tool)
        session.key('F10'); motion(703,403); record('fixed F10 opens menu from active tool',pointer()); session.shot('fixed-menu'); session.key('Escape')
        session.key('F3'); motion(120,150); record('Preferences uses normal pointer',pointer()); session.shot('preferences'); session.key('Escape')
        session.key('F1'); time.sleep(.25); record('explicit update action retains app and board',session.process.poll() is None); session.shot('manual-check')
        # Cancellation/menu access remains responsive even if the HTTPS request is outstanding.
        session.key('F10'); time.sleep(.3); record('fixed F10 interrupts update modal safely',session.process.poll() is None and pointer()); session.shot('check-to-menu')
        session.key('Escape'); motion(704,404); record('cancelled update preserves tool cursor',cursor.pixels()==tool)
        session.close(); report=json.loads(session.report.read_text()); session=None
        record('update cancellation has no board mutation',report['editing']['dirty'] is False and report['editing']['generation']==0 and report['editing']['undo_entries']==0)
        (root/'receipt.json').write_text(json.dumps(dict(checks=len(checks),passed=all(c['observed'] for c in checks),binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),build_info=json.loads(subprocess.check_output([str(binary),'--build-info'])),report=report),indent=2)+'\n')
    finally:
        if session and session.process.poll() is None: session.close(discard=True)
        if cursor: cursor.close()
        x.terminate(); x.wait(timeout=5); xlog.close()
if __name__=='__main__': main()
