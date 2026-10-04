#!/usr/bin/env python3
"""Convert harness-owned schema3 shape fixtures into ellipse probes; no private data."""
import argparse
import json
from pathlib import Path
import struct
import subprocess
import time
import zlib
from run_image_interaction import digest,distribution


def convert(path,target,full=False):
    data=path.read_bytes();count=struct.unpack_from('<I',data,56)[0]
    assert data[:8]==b'TACKSN01' and struct.unpack_from('<I',data,12)[0]==3
    assert struct.unpack_from('<II',data,48)==(0,0) and struct.unpack_from('<II',data,60)==(0,0)
    records=[];ids=[];p=96
    for _ in range(count):
        version,kind=struct.unpack_from('<HH',data,p);identity=data[p+4:p+20];n=struct.unpack_from('<I',data,p+20)[0];payload=bytearray(data[p+24:p+24+n][:67]);p+=24+n
        assert version==1 and 3<=kind<=8 and len(payload)==67
        if full:
            struct.pack_into('<dddd',payload,0,600.,300.,1200.,680.);payload[46]=1;payload[47:51]=bytes([255,198,82,255]);struct.pack_into('<dd',payload,51,30.,.5)
        records.append(struct.pack('<HH',1,5)+identity+struct.pack('<I',67)+payload);ids.append(identity)
        if full:break
    auth=data[80:96]+b''.join(records)+b''.join(ids)+struct.pack('<I',0)
    header=bytearray(data[:80]);struct.pack_into('<Q',header,16,len(auth));struct.pack_into('<Q',header,32,80+len(auth));struct.pack_into('<I',header,40,zlib.crc32(auth));struct.pack_into('<I',header,56,len(records));target.write_bytes(header+auth)


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',type=Path,required=True);p.add_argument('--fixtures',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args();root=args.output.resolve();root.mkdir(parents=True,exist_ok=False);runs=[]
    for name,count,full in [('ellipse-1000',1000,False),('ellipse-10000',10000,False),('ellipse-full',1000,True)]:
        board=root/f'{name}.tack';convert(args.fixtures/f'shapes-{count}.tack',board,full)
        subprocess.run([str(args.binary.resolve()),'inspect',str(board)],stdout=(root/f'{name}-inspect.json').open('w'),check=True,timeout=10)
        report=root/f'{name}.json';cmd=[str(args.binary.resolve()),'open',str(board),'--annotation-benchmark','--seconds','12','--output',str(report)];peak=0;started=time.monotonic()
        with (root/f'{name}.log').open('w') as log:
            child=subprocess.Popen(cmd,stdout=log,stderr=subprocess.STDOUT)
            try:
                while child.poll() is None:
                    if time.monotonic()-started>40:raise RuntimeError('ellipse deadline')
                    try:peak=max(peak,next(int(s.split()[1])*1024 for s in Path(f'/proc/{child.pid}/status').read_text().splitlines() if s.startswith('VmHWM:')))
                    except (FileNotFoundError,ProcessLookupError,StopIteration):pass
                    time.sleep(.02)
            finally:
                if child.poll() is None:child.terminate();child.wait(timeout=3)
        if child.returncode:raise RuntimeError('ellipse native failed')
        data=json.loads(report.read_text());frames=[f for f in data['frames'] if f['elapsed_ms']>=1000];assert len(frames)>=100 and data['source_bytes_before_detail']==0
        row={'name':name,'cpu':distribution([f['cpu_ms'] for f in frames]),'gpu':distribution([f['pass_ms'] for f in data['gpu_samples']]),'callback':distribution([f['callback_ms'] for f in frames]),'rss_peak_bytes':peak,'primitives':max(f['annotations']['primitives'] for f in frames),'report_sha256':digest(report),'board_sha256':digest(board),'command':cmd};runs.append(row);print(name,row['gpu'],flush=True)
    (root/'summary.json').write_text(json.dumps({'binary_sha256':digest(args.binary),'harness_sha256':digest(__file__),'runs':runs,'fixture':'only owned schema3 metadata, preserved IDs, no image payloads; full ellipse covers1200x680 with opaque fill/stroke globalopacity0.5'},indent=2)+'\n')


if __name__=='__main__':main()
