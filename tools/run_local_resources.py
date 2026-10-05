#!/usr/bin/env python3
"""Serial ordinary local windows: startup, idle, imports and recovery costs."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
from PIL import Image
from run_local_production import Session, wait
from run_idle import observe
from run_native_image_checks import command
from run_image_interaction import digest, distribution
from x11_drop import drop


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--prepared-board', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if os.environ.get('TACK_NATIVE_NO_WM') != '1' or os.environ.get('DISPLAY') in (None, ':0', ':0.0'):
        parser.error('isolated X11 required')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    env = dict(os.environ, TACK_PROFILE_DIR=str(root / 'profile'))
    results = {'binary_sha256': digest(binary), 'harness_sha256': digest(__file__), 'startup_idle': [], 'imports': [], 'autosave': [],
               'measurement': 'serial native X11; 5s idle intervals; /proc ticks/RSS/I/O, native submission receipts; no cold kernel-cache, allocator count or per-process VRAM attribution'}
    def publish():
        (root / 'summary.json').write_text(json.dumps(results, indent=2) + '\n')
    def delta(a, b):
        tasks = [{'name': t['name'], 'cpu_ticks': t['ticks'] - a['tasks'][tid]['ticks'], 'voluntary_switches': t['voluntary_ctxt_switches'] - a['tasks'][tid]['voluntary_ctxt_switches']} for tid, t in b['tasks'].items() if tid in a['tasks']]
        ticks = sum(t['cpu_ticks'] for t in tasks)
        return {'rss_initial_bytes': a['rss_bytes'], 'rss_final_bytes': b['rss_bytes'], 'threads': len(b['tasks']), 'tasks': tasks, 'cpu_ticks': ticks, 'cpu_percent_one_core': ticks / os.sysconf('SC_CLK_TCK') / 5 * 100, 'io_delta': {k: b['io'][k] - a['io'][k] for k in a['io']}}
    fixtures = root / 'images'
    fixtures.mkdir()
    small = []
    for i in range(64):
        p = fixtures / f'{i:03}.png'
        Image.new('RGB', (90 + i * 3, 80 + i % 7 * 19), (60 + i * 2, 110, 190 - i)).save(p)
        small.append(p)
    large = []
    for i in range(3):
        p = fixtures / f'large-{i}.png'
        Image.effect_noise((4000, 2500), 100).save(p)
        large.append(p)
    small_board = root / 'small.tack'
    subprocess.run([str(binary), 'create', str(small_board), '--embedded', *map(str, small[:8])], stdout=(root / 'create.log').open('w'), stderr=subprocess.STDOUT, check=True, timeout=30)
    prepared = args.prepared_board.resolve()
    empty_board = root / 'empty.tack'
    for name, arguments in [('empty', ['new', empty_board]), ('small', ['open', small_board]), ('prepared-1k', ['open', prepared])]:
        started = time.monotonic()
        s = Session(binary, root, name, arguments, env)
        spy = None
        try:
            time.sleep(2)
            title_log = root / f'{name}-title-properties.log'
            with title_log.open('w') as log:
                spy = subprocess.Popen(['xprop', '-spy', '-id', s.window, '_NET_WM_NAME', 'WM_NAME'], stdout=log, stderr=subprocess.STDOUT)
            wait(lambda: len(title_log.read_text().splitlines()) >= 2, 'initial owned title properties')
            initial_title_lines = len(title_log.read_text().splitlines())
            begin = time.monotonic() - started
            a = observe(s.process.pid)
            time.sleep(5)
            b = observe(s.process.pid)
            end = time.monotonic() - started
            spy.terminate()
            spy.wait(timeout=3)
            title_updates = len(title_log.read_text().splitlines()) - initial_title_lines
            title = s.title()
            s.close()
            report = json.loads(s.report.read_text())
            frames = [f for f in report['frames'] if begin * 1000 <= f['elapsed_ms'] <= end * 1000]
            row = {'name': name, **delta(a, b), **{k: report[k] for k in ('native_startup_ms', 'metadata_load_ms', 'first_frame_ms', 'first_recognizable_ms', 'ordinary_view_80_percent_ms', 'gpu_setup_ms', 'source_bytes_before_detail', 'redraw_count', 'wait_count')},
                   'idle_redraws': len(frames), 'idle_gpu_submissions': len(frames), 'idle_title_property_updates': title_updates, 'local': report['local'], 'title': title, 'report_sha256': digest(s.report)}
            if frames or title_updates or row['cpu_ticks'] > 5 or row['io_delta']['write_bytes']:
                raise AssertionError('idle gate')
            results['startup_idle'].append(row)
            publish()
            print('startup/idle', name, flush=True)
        finally:
            if spy is not None and spy.poll() is None:
                spy.terminate()
                spy.wait(timeout=3)
            s.kill()
    # Actual progressive native drops. Three 10MB noise PNGs exercise decoder
    # working sets; all files are generated before the timed windows start.
    for name, paths, cancel in [('one', small[:1], False), ('many', small, False), ('large', large, False), ('cancel', large * 40, True)]:
        board = root / f'import-{name}.tack'
        s = Session(binary, root, f'import-{name}', ['new', board], env)
        try:
            a = observe(s.process.pid)
            started = time.monotonic()
            drop(s.window, paths)
            if cancel:
                time.sleep(.12)
                s.key('Escape')
            peak, titles = 0, []
            def done():
                nonlocal peak
                status = Path(f'/proc/{s.process.pid}/status').read_text().splitlines()
                peak = max(peak, next(int(line.split()[1]) * 1024 for line in status if line.startswith('VmHWM:')))
                title = s.title()
                if title not in titles and len(titles) < 256:
                    titles.append(title)
                return ('modified' in title and 'Import ' not in title) or 'Import cancelled' in title
            wait(done, 'progressive import finish', seconds=35)
            wall = (time.monotonic() - started) * 1000
            time.sleep(1)
            saved = s.save(board)
            b = observe(s.process.pid)
            s.close()
            report = json.loads(s.report.read_text())
            row = {'name': name, 'requested_files': len(paths), 'admitted_images': sum(o['kind'] == 'image' for o in saved['objects']), 'encoded_bytes_requested': sum(p.stat().st_size for p in paths), 'import_wall_ms': wall, 'peak_rss_bytes': peak, 'io_delta': {k: b['io'][k] - a['io'][k] for k in a['io']}, 'progress_titles': titles, 'file_bytes': board.stat().st_size, 'local': report['local'], 'errors': report['errors'], 'overview_generated': report['overview_regenerated'], 'report_sha256': digest(s.report)}
            if (not cancel and row['admitted_images'] != len(paths)) or (cancel and row['admitted_images'] == len(paths)):
                raise AssertionError('admission/cancel gate')
            results['imports'].append(row)
            publish()
            print('import', name, flush=True)
        finally:
            s.kill()
    for name, input_board in [('small', small_board), ('prepared-1k', prepared)]:
        board = root / f'autosave-{name}.tack'
        shutil.copy2(input_board, board)
        normal = digest(board)
        s = Session(binary, root, f'autosave-{name}', ['open', board], env)
        try:
            time.sleep(1)
            a = observe(s.process.pid)
            s.key('ctrl+a', 'alt+shift+h')
            recovery = root / f'.{board.name}.tack-recovery'
            started = time.monotonic()
            wait(lambda: (recovery / 'state.meta').exists(), 'small/1k recovery published', seconds=12)
            delay = (time.monotonic() - started) * 1000
            b = observe(s.process.pid)
            # Camera input while the dirty document/recovery coexist is not an edit.
            command('xdotool', 'mousemove', '--window', s.window, '600', '300')
            command('xdotool', 'mousedown', '2')
            for i in range(20):
                command('xdotool', 'mousemove', '--window', s.window, str(600 + i * 2), '300')
                time.sleep(.02)
            command('xdotool', 'mouseup', '2')
            after = observe(s.process.pid)
            if digest(board) != normal:
                raise AssertionError('recovery overwrote normal file')
            recovery_bytes = sum(p.stat().st_size for p in recovery.iterdir() if p.is_file())
            # Acknowledged recovery remains inspectable until this explicit discard.
            s.close(discard=True)
            report = json.loads(s.report.read_text())
            row = {'name': name, 'debounce_to_publication_ms': delay, 'recovery_disk_bytes': recovery_bytes, 'write_bytes': b['io']['write_bytes'] - a['io']['write_bytes'], 'rss_before_bytes': a['rss_bytes'], 'rss_after_bytes': after['rss_bytes'], 'local': report['local'], 'editing': report['editing'], 'frames': len(report['frames']), 'callback': distribution([f['callback_ms'] for f in report['frames']]), 'normal_hash_unchanged': digest(board) == normal, 'edit_during_save': 'generation acknowledgement covered by deterministic worker integration test; native pan checked after publication', 'report_sha256': digest(s.report)}
            results['autosave'].append(row)
            publish()
            print('autosave', name, flush=True)
        finally:
            s.kill()


if __name__ == '__main__':
    main()
