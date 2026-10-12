#!/usr/bin/env python3
"""Stage A valid deterministic large assets, never padding/sparse fake images."""
import argparse, hashlib, json, random, shutil, struct
from pathlib import Path
from PIL import Image
p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args();root=a.output.resolve();root.mkdir(parents=True,exist_ok=False)
if shutil.disk_usage(root).free < 11*1024**3:raise RuntimeError('preserve 10 GiB disk reserve before generating corpus')
(root/'OWNER_STAGE_A.txt').write_text('P0-WIN-IMAGE-CORE-02 Stage A disposable fixtures\n')
rng=random.Random(261013);image=Image.frombytes('L',(8000,9000),rng.randbytes(8000*9000));rows=[]
for ext,kw in [('jpg',dict(quality=100,progressive=False,optimize=False)),('png',dict(compress_level=1))]:
 path=root/f'noise-8000x9000.{ext}';image.save(path,**kw)
 with Image.open(path) as check:
  assert check.size==(8000,9000) and check.mode=='L';progressive=bool(check.info.get('progressive') or check.info.get('progression'));check.verify()
 with path.open('rb') as f:sha=hashlib.file_digest(f,'sha256').hexdigest()
 row=dict(file=path.name,seed=261013,pixels=[8000,9000],mode='L',bit_depth=8,alpha=False,encoded_bytes=path.stat().st_size,sha256=sha,expected_current='metadata passes then 64 MiB encoded admission rejects; render NOT established')
 if ext=='jpg':row.update(progressive=progressive,encoder_options=kw)
 else:
  with path.open('rb') as f:header=f.read(33)
  assert header[:8]==b'\x89PNG\r\n\x1a\n' and header[12:16]==b'IHDR';row.update(interlace=header[28],color_type=header[25],bit_depth=header[24])
 assert row['encoded_bytes']>64*1024*1024
 rows.append(row)
(root/'fixtures.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(rows))
