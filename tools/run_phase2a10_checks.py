#!/usr/bin/env python3
"""Owned X11/WM desktop and real destination-picker checks, software Vulkan only."""
import argparse, hashlib, json, os, signal, subprocess, time
from pathlib import Path
from PIL import Image, ImageChops
from run_local_production import Session, wait
from run_native_image_checks import command
from run_phase2a_local_regression import snapshot, delta

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for n in ('binary', 'profile', 'output'):
        parser.add_argument('--' + n, type=Path, required=True)
    a = parser.parse_args()
    binary = a.binary.resolve()
    root = a.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    repo = Path(__file__).resolve().parents[1]
    xr = repo / 'target/native/tools/xvfb-root'
    env = dict(os.environ, DISPLAY=':125', TACK_NATIVE_NO_WM='1', TACK_TEST_WINDOW_SIZE='800x600', WINIT_X11_SCALE_FACTOR='1', LP_NUM_THREADS='2', LD_LIBRARY_PATH=str(xr / 'usr/lib/x86_64-linux-gnu'), VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json', WGPU_BACKEND='vulkan', XDG_CONFIG_HOME=str(root / 'wm-config'), DBUS_SESSION_BUS_ADDRESS='', GSK_RENDERER='cairo', GSETTINGS_BACKEND='memory', GIO_USE_VFS='local')
    os.environ.update(env)
    checks = []
    s = None
    wm = None
    xf = (root / 'xvfb.log').open('w')
    wf = (root / 'wm.log').open('w')
    if Path('/tmp/.X11-unix/X125').exists():
        raise RuntimeError('Display 125 is already occupied; refusing to drive it')
    xvfb = subprocess.Popen([str(xr / 'usr/bin/Xvfb'), ':125', '-screen', '0', '1600x1200x24', '-nolisten', 'tcp', '-noreset'], env=env, stdout=xf, stderr=subprocess.STDOUT)

    def record(name, ok, detail=None):
        checks.append(dict(name=name, observed=bool(ok), detail=detail))
        (root / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
        if not ok:
            raise AssertionError(name)

    def move(x, y):
        command('xdotool', 'mousemove', '--window', s.window, str(x), str(y))
        time.sleep(0.12)

    def click(x, y, button=1):
        move(x, y)
        command('xdotool', 'click', str(button))
        time.sleep(0.3)

    def geometry():
        return {k: int(v) for k, v in (line.split('=', 1) for line in command('xdotool', 'getwindowgeometry', '--shell', s.window).splitlines()) if k in ('X', 'Y', 'WIDTH', 'HEIGHT')}

    def state():
        return command('xprop', '-id', s.window, '_NET_WM_STATE')

    def shot(name):
        s.shot(name)
        return Image.open(root / f'{s.name}-{name}.png').convert('RGB')

    def drag(start, end, button):
        move(*start)
        command('xdotool', 'mousedown', str(button))
        time.sleep(0.12)
        for i in range(1, 7):
            move(round(start[0] + (end[0] - start[0]) * i / 6), round(start[1] + (end[1] - start[1]) * i / 6))
        command('xdotool', 'mouseup', str(button))
        time.sleep(0.35)
    try:
        wait(lambda: subprocess.run(['xdpyinfo'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0, 'owned display')
        wm = subprocess.Popen(['dbus-run-session', '--', 'xfwm4', '--compositor=off', '--sm-client-disable'], env=env, stdout=wf, stderr=subprocess.STDOUT, start_new_session=True)
        wait(lambda: '_NET_SUPPORTING_WM_CHECK(WINDOW)' in subprocess.check_output(['xprop', '-root', '_NET_SUPPORTING_WM_CHECK'], text=True), 'owned WM')
        source = root / 'source.png'
        Image.new('RGB', (480, 320), (225, 30, 70)).save(source)
        board = root / 'board.tack'
        subprocess.run([str(binary), 'create', str(board), '--linked', str(source)], env=env, check=True, capture_output=True, timeout=20)
        settings = json.loads(a.profile.read_text())
        settings.update(recent=[], local_views=[], native_title_bar=False, always_on_top=False, grid=False, grid_lines=False)
        settings['toolbar']['placement'] = 'Hidden'
        for entry in settings.get('toolbar_sets', []):
            entry['actions'] = settings['toolbar']['actions']
        remaps = {'F1': 'ToggleAlwaysOnTop', 'F2': 'ToggleNativeTitleBar', 'F3': 'ToggleGrid', 'F4': 'ExportSelectedSources', 'F5': 'ExportAllSources', 'F6': 'ExportBoardSnapshot', 'F7': 'Preferences'}
        settings['keymap'] = [b for b in settings['keymap'] if b['action'] not in remaps.values() and b['control'] not in [{'LogicalKey': {'Named': k}} for k in remaps]]
        for key, action in remaps.items():
            settings['keymap'].append(dict(action=action, control={'LogicalKey': {'Named': key}}, modifiers=0, modifier_match='Exact', trigger='Press'))
        profile = root / 'profile'
        profile.mkdir()
        prefs = profile / 'preferences.json'
        prefs.write_text(json.dumps(settings))
        env['TACK_PROFILE_DIR'] = str(profile)
        s = Session(binary, root, 'ux', ['open', board], env)
        time.sleep(1.0)
        move(40, 450)
        hidden = shot('hidden')
        hint = command('xprop', '-id', s.window, '_MOTIF_WM_HINTS')
        record('Compact window decoration request', '0x0' in hint or ', 0,' in hint, hint)
        move(400, 2)
        visible = shot('hover')
        record('Top edge reveals chrome', ImageChops.difference(hidden.crop((0, 0, 800, 24)), visible.crop((0, 0, 800, 24))).getbbox() is not None)
        move(400, 80)
        again = shot('hidden-again')
        record('Chrome hides on pointer departure', ImageChops.difference(hidden.crop((0, 0, 800, 24)), again.crop((0, 0, 800, 24))).getbbox() is None)
        move(16, 2)
        click(16, 12)
        menu = shot('mini-menu')
        record('Mini Tack menu opens', ImageChops.difference(hidden, menu).getbbox() is not None)
        s.key('Escape')
        s.key('F10')
        menu = shot('f10-menu')
        record('Fixed F10 still opens menu', ImageChops.difference(hidden, menu).getbbox() is not None)
        s.key('Escape')
        move(350, 2)
        before = geometry()
        drag((350, 12), (430, 62), 1)
        after = geometry()
        record('Native left chrome drag moves window', (before['X'], before['Y']) != (after['X'], after['Y']) and (before['WIDTH'], before['HEIGHT']) == (after['WIDTH'], after['HEIGHT']), dict(before=before, after=after))
        before = geometry()
        drag((40, 450), (90, 490), 3)
        after = geometry()
        fallback = shot('right-fallback')
        record('X11 right drag provides a nonmodal left-chrome fallback', before == after and ImageChops.difference(hidden.crop((200, 100, 600, 400)), fallback.crop((200, 100, 600, 400))).getbbox() is None and ImageChops.difference(hidden.crop((0, 578, 800, 600)), fallback.crop((0, 578, 800, 600))).getbbox() is not None, dict(before=before, after=after))
        before = geometry()
        click(40, 450, 3)
        right = shot('right-menu')
        record('Simple empty right click keeps context menu', ImageChops.difference(again.crop((0, 24, 800, 576)), right.crop((0, 24, 800, 576))).getbbox() is not None and geometry() == before)
        s.key('Escape')
        before = geometry()
        drag((40, 450), (44, 450), 3)
        small = shot('small-right-menu')
        record('Subthreshold right movement preserves menu', geometry() == before and ImageChops.difference(again.crop((0, 24, 800, 576)), small.crop((0, 24, 800, 576))).getbbox() is not None)
        s.key('Escape')
        before = geometry()
        drag((400, 300), (440, 330), 3)
        record('Object right drag never moves native window', geometry() == before)
        s.key('Escape')
        before = geometry()
        drag((799, 590), (819, 590), 1)
        after = geometry()
        record('Native edge resize changes width', after['WIDTH'] > before['WIDTH'], dict(before=before, after=after))
        command('xdotool', 'windowsize', s.window, '800', '600')
        time.sleep(0.3)
        s.key('F1')
        wait(lambda: json.loads(prefs.read_text())['always_on_top'], 'top setting')
        record('Always on Top reaches WM', '_NET_WM_STATE_ABOVE' in state(), state())
        s.key('F1')
        time.sleep(0.3)
        record('Always on Top disabled without polling', '_NET_WM_STATE_ABOVE' not in state(), state())
        s.key('F2')
        wait(lambda: json.loads(prefs.read_text())['native_title_bar'], 'title preference')
        record('Native title bar can be restored', json.loads(prefs.read_text())['native_title_bar'])
        s.key('F2')
        time.sleep(0.3)
        move(400, 2)
        click(760, 12)
        wait(lambda: geometry()['WIDTH'] > 800, 'maximized')
        record('Chrome maximize uses native state', '_NET_WM_STATE_MAXIMIZED_HORZ' in state(), geometry())
        width = geometry()['WIDTH']
        move(width - 40, 2)
        click(width - 40, 12)
        wait(lambda: geometry()['WIDTH'] == 800, 'restored')
        record('Chrome restore returns to initial width', geometry()['WIDTH'] == 800)
        move(400, 2)
        click(736, 12)
        wait(lambda: '_NET_WM_STATE_HIDDEN' in state(), 'minimized')
        record('Chrome minimize uses native state', True, state())
        subprocess.run(['xdotool', 'windowmap', s.window], check=True)
        subprocess.run(['xdotool', 'windowactivate', '--sync', s.window], check=True)
        s.focus()
        time.sleep(0.5)
        s.key('F3')
        s.key('F7')
        for _ in range(12):
            s.key('Down')
        s.key('Return')
        wait(lambda: json.loads(prefs.read_text())['grid_lines'], 'inline lines preference')
        shot('inline-grid-style')
        s.key('Escape')
        move(50, 450)
        shot('lines')
        record('Grid Lines setting is inline and persists', json.loads(prefs.read_text())['grid_lines'])
        s.key('F3')
        dest = root / 'exports'
        dest.mkdir()
        s.key('ctrl+a')
        s.picker('F4', dest, selects_file=False)
        wait(lambda: (dest / 'Tack Sources/source.png').is_file(), 'selected export')
        record('Real directory picker / selected Linked bytes', (dest / 'Tack Sources/source.png').read_bytes() == source.read_bytes())
        s.picker('F5', dest, selects_file=False)
        wait(lambda: (dest / 'Tack Sources_2/source.png').is_file(), 'all export')
        record('All sources do not replace prior export', (dest / 'Tack Sources_2/source.png').read_bytes() == source.read_bytes())
        png = root / 'board-snapshot.png'
        s.picker('F6', png, selects_file=False)
        wait(png.is_file, 'snapshot export', seconds=25)
        im = Image.open(png).convert('RGB')
        record('Real snapshot picker produces board pixels', im.getpixel((im.width // 2, im.height // 2)) == (225, 30, 70), dict(size=im.size))
        s.key('Escape')
        move(40, 450)
        time.sleep(2.0)
        before = snapshot(s.process.pid)
        time.sleep(3.0)
        idle = delta(before, snapshot(s.process.pid), 3.0)
        record('Desktop/export idle has zero CPU and I/O', idle['ticks'] == 0 and all((v == 0 for v in idle['io_delta'].values())), idle)
        move(400, 2)
        click(784, 12)
        s.process.wait(timeout=8)
        s.log.close()
        record('Chrome close exits the owned clean board', s.process.returncode == 0)
        report = json.loads(s.report.read_text())
        s = None
        settings = json.loads(prefs.read_text())
        settings['keymap'].append(dict(action='PanView', control={'Pointer': {'Mouse': 'Right'}}, modifiers=0, modifier_match='Exact', trigger='Hold'))
        prefs.write_text(json.dumps(settings))
        s = Session(binary, root, 'custom-right', ['open', board], env)
        time.sleep(0.5)
        move(40, 450)
        baseline = shot('custom-before')
        before = geometry()
        drag((40, 450), (100, 490), 3)
        after = geometry()
        moved = shot('custom-after')
        s.close(discard=True)
        custom = json.loads(s.report.read_text())
        s = None
        record('Custom right-button pan bypasses window fallback and menu', before == after and ImageChops.difference(baseline.crop((100, 100, 700, 500)), moved.crop((100, 100, 700, 500))).getbbox() is not None and ('Use left drag' not in (root / 'custom-right.log').read_text()), custom['desktop'])
        (root / 'receipt.json').write_text(json.dumps(dict(passed=True, checks=len(checks), binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), build_info=json.loads(subprocess.check_output([str(binary), '--build-info'])), idle=idle, desktop=report['desktop'], scope='Owned Xvfb125 + xfwm4; Lavapipe Vulkan two threads; physical Windows/Wayland remain explicit acceptance gaps. X11 right drag uses a bounded left-chrome fallback.'), indent=2) + '\n')
    finally:
        if s:
            try:
                s.close(discard=True)
            except Exception:
                s.kill()
        if wm:
            os.killpg(wm.pid, signal.SIGTERM)
            wm.wait(timeout=5)
        xvfb.terminate()
        xvfb.wait(timeout=5)
        xf.close()
        wf.close()
if __name__ == '__main__':
    main()
