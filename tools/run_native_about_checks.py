#!/usr/bin/env python3
"""Owned X11 About checks: pixels, both routes, dismissal and repeated resources."""
import argparse
import json
import os
import shutil
from pathlib import Path
import time
from PIL import Image
from run_local_production import Session
from run_native_image_checks import command
from run_image_interaction import digest
from run_idle import observe


def fault_checks(binary, root, board, env, record):
    """One disposable executable; never alter the user's package or source."""
    folder = root / 'faults'; folder.mkdir()
    copy = folder / 'tack'; shutil.copy2(binary, copy)
    asset = folder / 'tack-about.png'
    try:
        for case, payload in [('missing', None), ('corrupt', b'not PNG'), ('oversized', b'0' * (128 * 1024 + 1))]:
            if payload is not None: asset.write_bytes(payload)
            s = Session(copy, folder, case, ['open', board], dict(env, TACK_PROFILE_DIR=str(folder / ('profile-' + case))))
            try:
                s.key('F10', 'Up', 'Return'); time.sleep(.25); s.shot('about')
                s.key('Escape'); s.key('g'); s.close()
                report = json.loads(s.report.read_text())
                record('packaged artwork ' + case, report['about']['requests'] == 1 and report['about']['image_gpu_bytes'] == 0 and report['spatial']['grid'],
                       {'metadata_modal_close_canvas_resume': True, 'report': report['about'], 'capture_sha256': digest(folder / (case + '-about.png'))})
            finally:
                s.kill()
    finally:
        copy.unlink()
        if asset.exists(): asset.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--board', type=Path, required=True)
    parser.add_argument('--profile', type=Path, required=True)
    parser.add_argument('--asset', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('DISPLAY') in (None, ':0', ':0.0') or os.environ.get('TACK_NATIVE_NO_WM') != '1':
        parser.error('requires an owned isolated X11 display and TACK_NATIVE_NO_WM=1')
    root = args.output.resolve(); root.mkdir(parents=True, exist_ok=False)
    binary, board = args.binary.resolve(), args.board.resolve()
    source_hash = digest(board)
    profile = json.loads(args.profile.read_text())
    image = Image.open(args.asset).convert('RGB')
    checks = []
    receipt = {'binary_sha256': digest(binary), 'harness_sha256': digest(__file__),
               'board_sha256': source_hash, 'image_sha256': digest(args.asset),
               'scope': 'automated owned X11; owner visual review pending', 'checks': checks}
    def record(name, valid, detail):
        checks.append({'name': name, 'observed': bool(valid), 'detail': detail})
        (root / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
        if not valid: raise AssertionError(name)
    for scale in (1, 2):
        for theme in ('VeryDark', 'NeutralGray', 'Light'):
            case = f'{theme}-{scale}'
            config = root / 'profiles' / case; config.mkdir(parents=True)
            (config / 'preferences.json').write_text(json.dumps(dict(profile, theme=theme, ui_scale=scale, grid=False, recent=[])))
            env = dict(os.environ, TACK_PROFILE_DIR=str(config), TACK_TEST_WINDOW_SIZE='800x600')
            s = Session(binary, root, case, ['open', board], env)
            try:
                s.key('F10', 'Up', 'Return'); time.sleep(.25)
                s.shot('about-f10')
                actual = Image.open(root / f'{case}-about-f10.png').convert('RGB')
                # Independently computed 800x600 pixel rectangles, matched against actual GPU output.
                rect = (481, 188, 688, 412) if scale == 1 else (360, 88, 752, 512)
                expected = image.resize((207, 224) if scale == 1 else (392, 424), Image.Resampling.NEAREST)
                crop = actual.crop(rect)
                differences = [max(abs(a-b) for a,b in zip(p,q)) for p,q in zip(crop.getdata(),expected.getdata())]
                near = sum(v <= 1 for v in differences) / len(differences)
                record(case + ' artwork and aspect', near > .995 and actual.size == (800,600), {'matching_fraction': near, 'rect': rect})
                s.key('Escape'); s.shot('closed')
                closed = Image.open(root / f'{case}-closed.png').convert('RGB')
                record(case + ' Escape', closed.tobytes() != actual.tobytes(), 'canvas restored')
                command('xdotool', 'mousemove', '--window', s.window, '250', '180')
                command('xdotool', 'click', '3'); s.key(*(['Down'] * 7), 'Return'); time.sleep(.25)
                s.shot('about-context')
                context = Image.open(root / f'{case}-about-context.png').convert('RGB')
                record(case + ' same modal through context', context.tobytes() == actual.tobytes(), 'pixel-identical About from shared Tack entry')
                s.key('Return'); s.shot('enter-closed')
                record(case + ' Enter', Image.open(root / f'{case}-enter-closed.png').convert('RGB').tobytes() == closed.tobytes(), 'canvas restored')
                s.key('F10', 'Up', 'Return'); time.sleep(.25)
                x, y = (656,426) if scale == 1 else (688,540)
                command('xdotool', 'mousemove', '--window', s.window, str(x), str(y)); command('xdotool', 'click', '1'); time.sleep(.15)
                s.shot('button-closed')
                record(case + ' Close button', Image.open(root / f'{case}-button-closed.png').convert('RGB').tobytes() == closed.tobytes(), {'point': [x,y]})
                if theme == 'NeutralGray' and scale == 2:
                    samples = []
                    for batch in range(4):
                        for _ in range(10):
                            s.key('F10', 'Up', 'Return'); s.key('Escape')
                        time.sleep(.3)
                        samples.append(observe(s.process.pid))
                    rss = [sample['rss_bytes'] for sample in samples]
                    record('40 additional cycles bounded RSS', max(rss)-min(rss) < 4*1024*1024 and all(not any(t['name'].startswith('tack-local') for t in sample['tasks'].values()) for sample in samples), {'rss': rss, 'local_workers': [sum(t['name']=='tack-local-oper' for t in v['tasks'].values()) for v in samples]})
                s.close()
                report = json.loads(s.report.read_text())
                wanted = 43 if theme == 'NeutralGray' and scale == 2 else 3
                record(case + ' resources retired', report['about']['requests']==wanted and report['about']['image_gpu_bytes']==0, report['about'])
            finally:
                s.kill()
    fault_checks(binary, root, board, dict(os.environ, TACK_TEST_WINDOW_SIZE='800x600'), record)
    record('board untouched', digest(board)==source_hash, source_hash)


if __name__ == '__main__': main()
