#!/usr/bin/env python3
"""Nested Weston/X11 compositor on owned Xvfb126; never drive the user's desktop."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import time
from PIL import Image, ImageChops
from run_local_production import wait
from run_native_image_checks import command


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('binary', 'profile', 'board', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    repo = Path(__file__).resolve().parents[1]
    xvroot = repo / 'target/native/tools/xvfb-root'
    weston = repo / 'target/native/tools/weston-root'
    lib = weston / 'usr/lib/x86_64-linux-gnu'
    runtime = root / 'runtime'
    runtime.mkdir(mode=0o700)
    profile = root / 'profile'
    profile.mkdir()
    prefs = profile / 'preferences.json'
    settings = json.loads(args.profile.read_text())
    settings.update(native_title_bar=False, always_on_top=False, recent=[], local_views=[])
    prefs.write_text(json.dumps(settings))
    env = dict(os.environ, DISPLAY=':126', XDG_RUNTIME_DIR=str(runtime),
               LD_LIBRARY_PATH=f'{lib}/weston:{lib}:{xvroot}/usr/lib/x86_64-linux-gnu',
               WESTON_MODULE_MAP=f'x11-backend.so={lib}/libweston-13/x11-backend.so',
               VK_DRIVER_FILES='/usr/share/vulkan/icd.d/lvp_icd.json',
               WGPU_BACKEND='vulkan', LP_NUM_THREADS='2')
    os.environ.update(env)
    if Path('/tmp/.X11-unix/X126').exists():
        raise RuntimeError('Display 126 is occupied; refusing to drive it')
    compositor = app = None
    checks = []
    def record(name, observed):
        checks.append(dict(name=name, observed=bool(observed)))
        (root / 'checks.json').write_text(json.dumps(checks, indent=2) + '\n')
        if not observed:
            raise AssertionError(name)
    with (root / 'xvfb.log').open('w') as xf, (root / 'weston.stdout.log').open('w') as wf, (root / 'app.log').open('w') as af:
        display = subprocess.Popen([str(xvroot / 'usr/bin/Xvfb'), ':126', '-screen', '0', '1024x768x24', '-nolisten', 'tcp'], env=env, stdout=xf, stderr=subprocess.STDOUT)
        try:
            wait(lambda: display.poll() is None and subprocess.run(['xdpyinfo'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0, 'owned display')
            compositor = subprocess.Popen([str(weston / 'usr/bin/weston'), '--backend=x11-backend.so', f'--shell={lib}/weston/kiosk-shell.so', '--renderer=pixman', '--width=800', '--height=600', '--no-config', '--idle-time=0', '--socket=tack-2a10', f'--log={root}/weston.log'], env=env, stdout=wf, stderr=subprocess.STDOUT)
            wait(lambda: compositor.poll() is None and (runtime / 'tack-2a10').exists(), 'owned Weston')
            def mapped():
                text = (root / 'weston.log').read_text()
                match = re.search(r'x11 output .*window id (\d+)', text)
                return match.group(1) if match else None
            window = wait(mapped, 'compositor X11 window')
            wait(lambda: 'IsViewable' in command('xwininfo', '-id', window), 'owned compositor mapped')
            command('xdotool', 'windowfocus', '--sync', window)
            client_env = dict(env, WAYLAND_DISPLAY='tack-2a10', TACK_PROFILE_DIR=str(profile))
            client_env.pop('DISPLAY', None)
            app = subprocess.Popen([str(binary), 'open', str(args.board.resolve()), '--seconds', '12', '--output', str(root / 'native.json')], env=client_env, stdout=af, stderr=subprocess.STDOUT)
            time.sleep(1.)
            def move(x, y):
                command('xdotool', 'mousemove', '--window', window, str(x), str(y))
                time.sleep(.25)
            def shot(name):
                path = root / (name + '.png')
                subprocess.run(['import', '-window', window, str(path)], check=True, timeout=4)
                return Image.open(path).convert('RGB')
            move(400, 100)
            hidden = shot('hidden')
            command('xdotool', 'key', 'F10')
            time.sleep(.3)
            menu = shot('f10')
            record('Wayland fixed F10 opens application menu', ImageChops.difference(hidden, menu).getbbox() is not None)
            command('xdotool', 'key', 'Escape', 'F1')
            time.sleep(.4)
            record('Wayland unsupported Top returns an explicit message', 'Always on Top is unavailable on Wayland' in (root / 'app.log').read_text())
            command('xdotool', 'key', 'Escape', 'F2')
            wait(lambda: json.loads(prefs.read_text())['native_title_bar'], 'native preference')
            move(400, 100)
            native_hidden = shot('native-hidden')
            move(400, 2)
            hover = shot('native-hover')
            record('Wayland keeps hover controls when native CSD is absent', ImageChops.difference(native_hidden.crop((0, 0, 800, 24)), hover.crop((0, 0, 800, 24))).getbbox() is not None)
            app.wait(timeout=15)
            report = json.loads((root / 'native.json').read_text())
            record('Native Wayland renderer exits cleanly', app.returncode == 0 and report['desktop']['wayland'])
            (root / 'receipt.json').write_text(json.dumps(dict(passed=True, checks=checks, desktop=report['desktop'], binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), scope='Owned Xvfb126, nested Weston kiosk shell/pixman, native Wayland winit, Lavapipe Vulkan; timed smoke only, not an idle or physical compositor/move acceptance.'), indent=2) + '\n')
        finally:
            for process in (app, compositor, display):
                if process is not None and process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)


if __name__ == '__main__':
    main()
