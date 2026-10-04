#!/usr/bin/env python3
"""Phase1E generated annotations and common-canvas timings; serial GPU work."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import time
import zipfile
from run_image_interaction import digest, distribution


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,default=Path('target/release/tack-app'))
    parser.add_argument('--prepared-board',type=Path)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--seconds',type=float,default=12)
    args=parser.parse_args()
    if not 3<=args.seconds<=120:parser.error('duration 3..120 seconds')
    root=args.output.resolve();root.mkdir(parents=True,exist_ok=False);binary=root/'tack-app';shutil.copy2(args.binary,binary)
    project=Path(__file__).resolve().parent.parent
    paths=[project/n for n in ('Cargo.toml','Cargo.lock','rust-toolchain.toml')]
    paths += [p for p in (project/'crates').rglob('*') if p.suffix in ('.rs','.wgsl','.toml')]
    paths += list((project/'tools').glob('*.py'))+list((project/'.cargo').glob('*.toml'))
    paths += [p for p in (project/'assets').rglob('*') if p.is_file()]
    with zipfile.ZipFile(root/'source-snapshot.zip','w',zipfile.ZIP_DEFLATED) as archive:
        for p in sorted(paths):archive.write(p,p.relative_to(project))
    command=[str(binary),'annotation-scale',str(root/'metadata')]
    if args.prepared_board:command.append(str(args.prepared_board.resolve()))
    subprocess.run(command,stdout=(root/'metadata.log').open('w'),stderr=subprocess.STDOUT,check=True,timeout=60)
    cases=['shapes-1000','shapes-5000','shapes-10000','text-100','text-1000','scribble-1','scribble-2','scribble-100']
    if args.prepared_board:cases.append('mixed')
    runs=[]
    for name in cases:
        board=root/'metadata'/f'{name}.tack';report=root/f'{name}.json'
        cmd=[str(binary),'open',str(board),'--annotation-benchmark','--seconds',str(args.seconds),'--output',str(report)]
        peak=0;started=time.monotonic()
        with (root/f'{name}.log').open('w') as log:
            p=subprocess.Popen(cmd,stdout=log,stderr=subprocess.STDOUT)
            try:
                while p.poll() is None:
                    if time.monotonic()-started>args.seconds+30:raise RuntimeError('bounded annotation deadline')
                    try:
                        status=Path(f'/proc/{p.pid}/status').read_text().splitlines()
                        peak=max(peak,next(int(s.split()[1])*1024 for s in status if s.startswith('VmHWM:')))
                    except (FileNotFoundError,ProcessLookupError,StopIteration):pass
                    time.sleep(.02)
            finally:
                if p.poll() is None:p.terminate();p.wait(timeout=3)
        if p.returncode:raise RuntimeError(f'{name} returned {p.returncode}')
        data=json.loads(report.read_text());frames=data['frames'];steady=[f for f in frames if f['elapsed_ms']>=1000]
        if len(steady)<100:raise AssertionError('insufficient samples')
        if data['source_bytes_before_detail']!=0:raise AssertionError('annotation/prepared rendering read originals')
        if any(f['annotations'] and f['annotations']['omitted'] for f in steady):raise AssertionError('unexpected annotation omission')
        row={'name':name,'command':cmd,'rss_peak_bytes':peak,'cpu':distribution([f['cpu_ms'] for f in steady]),'gpu':distribution([f['pass_ms'] for f in data['gpu_samples']]),'callback':distribution([f['callback_ms'] for f in steady]),'layout':distribution([f['annotations']['layout_ms'] for f in steady if f['annotations']]),'build':distribution([f['annotations']['build_ms'] for f in steady if f['annotations']]),'peak_primitives':max(f['annotations']['primitives'] for f in steady if f['annotations']),'peak_glyphs':max(f['annotations']['glyphs'] for f in steady if f['annotations']),'first_frame_cpu_ms':frames[0]['cpu_ms'],'annotation_resources':data['annotation_resources'],'source_bytes':data['source_bytes_before_detail'],'report_sha256':digest(report),'board_sha256':digest(board)}
        runs.append(row);print(name,'CPU p99',row['cpu']['p99_ms'],'GPU p99',row['gpu']['p99_ms'],flush=True)
    summary={'binary_sha256':digest(binary),'source_snapshot_sha256':digest(root/'source-snapshot.zip'),'harness_sha256':digest(__file__),'metadata':json.loads((root/'metadata/metadata.json').read_text()),'runs':runs,'percentile':'sorted floor(p*(n-1)); max retained','timing':'steady CPU/callback after 1s; GPU all pass samples; build includes cull/packet/layout, layout subset; first frame includes lazy shader/font init','visibility':'fixed camera [600,300] zoom1; generated dense tiny shape/note stress, not artist corpus; text5000 metadata/layout only'}
    (root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')


if __name__=='__main__':main()
