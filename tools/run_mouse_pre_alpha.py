#!/usr/bin/env python3
"""Owned X11/software Vulkan Mulot checks; fake deadline, no physical desktop."""
import argparse, hashlib, json, os, subprocess, time
from pathlib import Path
from PIL import Image
from run_local_production import Session, wait
from run_native_image_checks import command
from run_native_annotation_checks import saved_objects
from run_phase2a8_foundation import Cursor, expected
from run_phase2a_local_regression import snapshot, delta


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'profile', 'output'):
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args(); binary = a.binary.resolve(); root = a.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    repo = Path(__file__).resolve().parents[1]; xroot = repo / 'target/native/tools/xvfb-root'
    env = dict(os.environ, DISPLAY=':120', TACK_NATIVE_NO_WM='1', TACK_NATIVE_DIAGNOSTICS='1',
               WINIT_X11_SCALE_FACTOR='1', LP_NUM_THREADS='2', TACK_TEST_WINDOW_SIZE='800x600',
               LD_LIBRARY_PATH=str(xroot / 'usr/lib/x86_64-linux-gnu'),
               VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json', WGPU_BACKEND='vulkan')
    os.environ.update(env)
    log = (root / 'xvfb.log').open('w')
    x = subprocess.Popen([str(xroot / 'usr/bin/Xvfb'), ':120', '-screen', '0', '1280x1024x24',
                          '-nolisten', 'tcp', '-noreset'], env=env, stdout=log, stderr=subprocess.STDOUT)
    s = None; cursor = None; checks = []; idle_windows = []
    def record(name, ok, detail=None):
        checks.append(dict(name=name, observed=bool(ok), detail=detail))
        (root / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
        if not ok: raise AssertionError(name)
    def move(px, py, pause=.06):
        command('xdotool', 'mousemove', '--window', s.window, str(px), str(py)); time.sleep(pause)
    def click(px, py):
        move(px, py); command('xdotool', 'click', '1'); time.sleep(.12)
    def read(): return json.loads((profile / 'preferences.json').read_text())
    def idle(name, seconds=2):
        start = snapshot(s.process.pid); time.sleep(seconds); end = snapshot(s.process.pid)
        row = delta(start, end, seconds); idle_windows.append(dict(name=name, receipt=row))
        record(name + ': no CPU ticks or main-loop wakeups', row['ticks'] == 0 and
               next(t['voluntary_switches'] for t in row['tasks'] if t['name'] == 'tack') == 0, row)
        record(name + ': no new disk writes or LAN sockets', row['io_delta']['write_bytes'] == 0 and
               not any(v['kind'] in ('tcp', 'tcp6', 'udp', 'udp6') for v in row['socket_records']), row)
    try:
        wait(lambda: subprocess.run(['xdpyinfo'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0, 'owned display')
        cursor = Cursor()
        fixture = root / 'fixture.png'; Image.new('RGB', (160,100), (40,120,190)).save(fixture)
        board = root / 'board.tack'
        subprocess.run([str(binary), 'create', str(board), '--embedded', str(fixture)], check=True, capture_output=True, timeout=20)
        settings = json.loads(a.profile.read_text()); settings.update(recent=[], local_views=[], status_bar=True)
        settings['toolbar'].update(scale=1, placement='Top', edge_position=0)
        settings['toolbar']['actions'] = [v for v in settings['toolbar']['actions'] if v != 'SelectTool(Mouse)']
        remaps = {'F1':'SelectTool(Mouse)', 'F2':'SelectTool(Pointer)', 'F3':'Preferences', 'F4':'KeymapEditor', 'F5':'EditToolbar'}
        settings['keymap'] = [b for b in settings['keymap'] if b['action'] not in remaps.values() and b['control'] not in [{'LogicalKey':{'Named':key}} for key in remaps]]
        for key, action in remaps.items(): settings['keymap'].append(dict(action=action, control={'LogicalKey':{'Named':key}}, modifiers=0, modifier_match='Exact', trigger='Press'))
        profile = root / 'profile'; profile.mkdir(); (profile / 'preferences.json').write_text(json.dumps(settings))
        s = Session(binary, root, 'mouse', ['open', board], dict(env, TACK_PROFILE_DIR=str(profile), TACK_TEST_MOUSE_EASTER_MS='5000'))
        time.sleep(.7); s.key('Escape', 'F3')
        for theme in ('VeryDark', 'NeutralGray', 'Light'):
            if theme == 'VeryDark': s.key('Down', 'Down', 'Down')
            else: s.key('Down')
            s.key('Return'); wait(lambda: read()['theme'] == theme, 'inline theme persistence')
            s.shot('theme-' + theme); record('inline theme preview persisted: ' + theme, read()['theme'] == theme)
        s.key('Escape', 'F4'); s.text('Mulot'); s.shot('keymap'); s.key('Escape', 'Escape', 'F5')
        # Catalog starts Separator, Mouse. Enter is the normal Add command.
        click(120, 106); s.key('Return'); time.sleep(.3)
        record('Mouse manually added in Edit Toolbar', 'SelectTool(Mouse)' in read()['toolbar']['actions'])
        s.key('Escape'); move(600,400); s.key('F1'); move(601,401)
        record('canvas cursor exact authored minimulot', cursor.pixels() == expected(binary.parent / 'gfx/cursors/minimulot.png', [7,7]))
        move(25,10)
        record('toolbar cursor exact normal pointer', cursor.pixels() == expected(binary.parent / 'gfx/cursors/cursor_pointer.png', [1,1]))
        move(450,340)
        record('return to canvas restores minimulot', cursor.pixels() == expected(binary.parent / 'gfx/cursors/minimulot.png', [7,7]))
        for dx,dy in [(0,-40),(40,-40),(40,0),(40,40),(0,40),(-40,40),(-40,0),(-40,-40)]:
            move(450+dx,340+dy); move(450,340)
        s.shot('paws'); time.sleep(.9)
        command('xdotool','click','1'); s.shot('ping'); time.sleep(.5)
        s.key('F2'); time.sleep(5.2)
        first = s.save(board); record('early deactivation cancels egg and trail/ping make no object', len(first['objects']) == 1, first)
        move(450,340); s.key('F1'); time.sleep(5.7)
        one = s.save(board); record('accelerated continuous activation inserts one normal image', len(one['objects']) == 2 and all(o['kind'] == 'image' for o in one['objects']), one)
        time.sleep(5.3); record('same activation never repeats', s.save(board) == one)
        s.key('ctrl+z'); record('one Undo removes insertion', s.save(board) == first)
        s.key('ctrl+y'); record('Redo restores exact image', s.save(board) == one)
        move(498,372); time.sleep(.9); s.shot('hover-poo'); s.close(); report = json.loads(s.report.read_text()); s = None
        record('exact status on generated image', report['chrome']['status_text'] == 'Puzzo puzzo !', report['chrome']['status_text'])
        record('save/reopen ordinary image data', saved_objects(board) == one)
        # Real idle: launch without --output/--seconds (diagnostics would poll).
        settings = read(); settings['theme'] = 'NeutralGray'; (profile/'preferences.json').write_text(json.dumps(settings))
        s = Session(binary, root, 'idle', [], dict(env, TACK_PROFILE_DIR=str(profile)))
        time.sleep(1.2); s.key('Escape'); move(600,400); time.sleep(.9); idle('inactive settled')
        s.key('F1'); move(450,340); move(500,340); time.sleep(1.1); idle('active stationary before real ten-minute deadline')
        s.key('F2'); time.sleep(1.); idle('deactivated settled')
        s.close(); s = None
        (root/'receipt.json').write_text(json.dumps(dict(checks=len(checks), passed=all(c['observed'] for c in checks),
            binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), build_info=json.loads(subprocess.check_output([str(binary),'--build-info'])),
            idle_windows=idle_windows, report=report, manual_pending=['Real ten-minute human activation', 'Artist assessment of gait/directions', 'Physical Windows desktop']), indent=2)+'\n')
    finally:
        if s: s.kill()
        if cursor: cursor.close()
        x.terminate(); x.wait(timeout=5); log.close()
if __name__ == '__main__': main()
