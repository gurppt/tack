#!/usr/bin/env python3
"""Fresh forensic package only. Never replaces an installed Tack or creates a release."""
import argparse, datetime, hashlib, json, shutil, subprocess, zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def digest(p):
 with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary-dir',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--platform',choices=['linux','windows'],required=True);a=p.parse_args()
 output=a.output.resolve();output.mkdir(parents=True,exist_ok=True);folder=output/f'Tack-DEV-DIAG-WINLAN-01-{a.platform}-x64';folder.mkdir(exist_ok=False)
 ext='.exe' if a.platform=='windows' else '';stripper='x86_64-w64-mingw32-strip' if ext else 'strip'
 for src,dst in [('tack-app','tack'),('tack-server','tack-server'),('tack-updater','tack-updater'),('tack-diagnostics','tack-diagnostics')]:
  shutil.copy2(a.binary_dir/(src+ext),folder/(dst+ext));subprocess.run([stripper,'--strip-all',str(folder/(dst+ext))],check=True)
 native=ROOT/'target/native'/('libjpeg-turbo-3.2.0-windows-gnu' if ext else 'libjpeg-turbo-3.2.0');provenance=json.loads((native/'native-build.json').read_text())
 decoder=native/'bin'/('djpeg.exe' if ext else 'djpeg');assert digest(decoder)==provenance['decoder_sha256'];shutil.copy2(decoder,folder/('tack-jpeg-decoder'+ext))
 for f in a.binary_dir.glob('tack-icon*'):shutil.copy2(f,folder/f.name)
 for name in ['tack-about.png','tack-about-logo.png']:shutil.copy2(a.binary_dir/name,folder/name)
 for group in ['icons','cursors','mouse-game']:
  dest=folder/'gfx'/group;dest.mkdir(parents=True)
  for f in (ROOT/'gfx'/group).glob('*.png'):shutil.copy2(f,dest/f.name)
 notices=folder/'LICENSES';notices.mkdir()
 for f in (native/'notices').iterdir():
  if f.is_file():shutil.copy2(f,notices/f.name)
 for source,name in [('assets/pixel-font/COPYRIGHT.txt','pixel-font-COPYRIGHT.txt'),('assets/pixel-font/OFL-1.1.txt','pixel-font-OFL-1.1.txt'),('assets/ui-font/LICENSE','ui-font-LICENSE'),('assets/licenses/webpki-roots-CDLA-Permissive-2.0.txt','roots-LICENSE')]:shutil.copy2(ROOT/source,notices/name)
 for f in (ROOT/'tools/diagnostics').iterdir():
  if f.suffix==('.cmd' if ext else '.sh') or f.suffix=='.txt':
   shutil.copy2(f,folder/f.name)
   if f.suffix=='.sh':(folder/f.name).chmod(0o755)
 if not ext:
  (folder/'lib').mkdir();shutil.copy2((ROOT/'target/native/libXi-1.8.3/lib/libXi.so.6').resolve(),folder/'lib/libXi.so.6');shutil.copy2(ROOT/'target/native/libXi-1.8.3/COPYING',notices/'libXi-COPYING')
 imports={}
 if ext:
  for f in folder.glob('*.exe'):
   text=subprocess.check_output(['x86_64-w64-mingw32-objdump','-p',str(f)],text=True);dll=[line.split(':',1)[1].strip() for line in text.splitlines() if 'DLL Name:' in line];imports[f.name]=dll
   if any(n.lower().startswith(('libgcc','libstdc++','libwinpthread','turbojpeg')) for n in dll):raise RuntimeError('Unbundled runtime DLL')
 diff=subprocess.check_output(['git','diff','HEAD','--binary'],cwd=ROOT)
 (folder/'BUILD.json').write_text(json.dumps(dict(mission='DEV-DIAG-WINLAN-01',label='FORENSIC / INSTRUMENTATION ONLY / NOT A RELEASE',base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),dirty=bool(diff),patch_sha256=hashlib.sha256(diff).hexdigest(),utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),platform=a.platform,physical_windows_test='UNTESTED',imports=imports,native={k:provenance.get(k) for k in ["version","target","source_sha256","decoder_sha256"]},simd_required=True,nasm_used=bool(provenance.get("nasm")),files={str(f.relative_to(folder)):dict(bytes=f.stat().st_size,sha256=digest(f)) for f in folder.rglob('*') if f.is_file()}),indent=2)+'\n')
 archive=output/(folder.name+'.zip')
 with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
  for f in sorted(folder.rglob('*')):
   if f.is_file():z.write(f,f.relative_to(output))
 record=dict(path=str(archive),bytes=archive.stat().st_size,sha256=digest(archive));print(json.dumps(record))
 (output/(folder.name+'.sha256')).write_text(f"{record['sha256']}  {archive.name}\n")
if __name__=='__main__':main()
