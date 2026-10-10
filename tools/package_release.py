#!/usr/bin/env python3
"""Create a fresh portable alpha package and immutable asset manifest, never user data."""
import argparse,hashlib,json,os,re,shutil,subprocess,tomllib,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
MAX_ARCHIVE=128*1024*1024

def digest(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for b in iter(lambda:f.read(65536),b''):h.update(b)
 return h.hexdigest()

def validate_identity(record):
 if record.get('schema')!=1 or record.get('protocol_major')!=2:raise ValueError('Manifest schema/protocol mismatch')
 if not re.fullmatch(r'[0-9a-f]{40}',record.get('git_sha','')):raise ValueError('Invalid commit identity')
 version=record.get('version',''); channel=record.get('channel')
 number=r'(?:0|[1-9][0-9]*)'
 core=rf'{number}\.{number}\.{number}'
 pattern=core+rf'-dev\.{number}' if channel=='dev' else core if channel=='stable' else r'(?!)'
 if len(version)>64 or not re.fullmatch(pattern,version):raise ValueError('Version/channel mismatch')

def package(binary_dir,platform,output,channel,runner=(),require_clean=True):
 version=tomllib.loads((ROOT/'Cargo.toml').read_text())['workspace']['package']['version']
 sha=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
 dirty=bool(subprocess.check_output(['git','diff','HEAD','--name-only'],cwd=ROOT))
 if require_clean and dirty:raise ValueError('Release packaging requires committed tracked source')
 validate_identity(dict(schema=1,protocol_major=2,git_sha=sha,version=version,channel=channel))
 output.mkdir(parents=True,exist_ok=True)
 folder=output/('tack-'+platform)
 if folder.exists():raise ValueError('Package exists; immutable candidate output must be fresh')
 folder.mkdir()
 ext='.exe' if platform=='windows-x86_64' else ''
 identity={}
 for source,target in [('tack-app','tack'),('tack-server','tack-server'),('tack-updater','tack-updater')]:
  executable=binary_dir/(source+ext)
  data=json.loads(subprocess.check_output([*runner,str(executable),'--build-info'],timeout=30))
  if any(data.get(k)!=v for k,v in [('version',version),('git_sha',sha),('channel',channel),('protocol_major',2)]):raise ValueError('Compiled package identity mismatch: '+source)
  identity[source]=data
  shutil.copy2(executable,folder/(target+ext))
  if not ext and shutil.which('strip'):subprocess.run(['strip','--strip-all',str(folder/target)],check=True)
 decoder=binary_dir/('tack-jpeg-decoder'+ext)
 shutil.copy2(decoder,folder/decoder.name)
 for name in ['tack-about.png','tack-about-logo.png','tack-icon-16.png','tack-icon-32.png','tack-icon-64.png','tack-icon-128.png','tack-icon-256.png','tack-icon.ico']:
  shutil.copy2(binary_dir/name,folder/name)
 for group in ['icons','cursors']:
  target=folder/'gfx'/group;target.mkdir(parents=True)
  for source in sorted((ROOT/'gfx'/group).glob('*.png')):shutil.copy2(source,target/source.name)
 notices=folder/'LICENSES';notices.mkdir()
 for source in [ROOT/'assets/pixel-font/COPYRIGHT.txt',ROOT/'assets/pixel-font/OFL-1.1.txt',ROOT/'assets/ui-font/LICENSE']:
  shutil.copy2(source,notices/('font-'+source.name))
 shutil.copy2(ROOT/'assets/licenses/webpki-roots-CDLA-Permissive-2.0.txt',notices/'webpki-roots-CDLA-Permissive-2.0.txt')
 native=ROOT/'target/native'/('libjpeg-turbo-3.2.0-windows-gnu' if platform=='windows-x86_64' and os.name!='nt' else 'libjpeg-turbo-3.2.0')
 provenance=json.loads((native/'native-build.json').read_text())
 if provenance.get('source_sha256')!='6f30092cef9fb839779646608f4ee14ae3cbac989c47fa05e841b0841f09878e' or not provenance.get('nasm') or '-DREQUIRE_SIMD=ON' not in provenance.get('cmake_command',[]):raise ValueError('Pinned SIMD decoder provenance missing')
 if digest(decoder)!=provenance.get('decoder_sha256'):raise ValueError('Decoder differs from verified native build')
 if not ext:
  library=ROOT/'target/native/libXi-1.8.3/lib/libXi.so.6'
  (folder/'lib').mkdir();shutil.copy2(library.resolve(),folder/'lib/libXi.so.6')
  shutil.copy2(ROOT/'target/native/libXi-1.8.3/COPYING',notices/'libXi-COPYING')
 for source in (native/'notices').iterdir():
  if source.is_file():shutil.copy2(source,notices/source.name)
 (folder/'README.txt').write_text('Tack '+version+'\nPortable alpha: keep this folder together. Run tack'+ext+'.\nManual updates only. Boards and profiles stay outside release assets.\nEditable PNG icons/cursors survive updates. License to be defined. Trusted LAN only.\n',encoding='utf8')
 record=dict(version=version,commit=sha,channel=channel,protocol_major=identity['tack-app']['protocol_major'],worktree_dirty=dirty,executables=identity,native=provenance)
 (folder/'BUILD.json').write_text(json.dumps(record,indent=2)+'\n')
 archive=output/('tack-'+platform+'.zip')
 if archive.exists():raise ValueError('Archive already exists')
 with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
  for source in sorted(folder.rglob('*')):
   if source.is_file():z.write(source,source.relative_to(output))
 size=archive.stat().st_size
 if size>MAX_ARCHIVE:raise ValueError('Update archive bound exceeded')
 fragment=dict(schema=1,version=version,channel=channel,git_sha=sha,protocol_major=record['protocol_major'],assets=[dict(platform=platform,name=archive.name,size=size,sha256=digest(archive))])
 (output/('manifest-'+platform+'.json')).write_text(json.dumps(fragment,indent=2)+'\n')
 return fragment

def merge(fragments,output):
 records=[]
 for p in fragments:
  if p.stat().st_size>32768:raise ValueError('Manifest fragment size limit')
  records.append(json.loads(p.read_text()))
  validate_identity(records[-1])
 if len(records)!=2:raise ValueError('Both platforms required')
 base=records[0]
 intended=subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip()
 if base["git_sha"]!=intended:raise ValueError("Manifest does not identify the release workflow commit")
 for other in records[1:]:
  if any(base[k]!=other[k] for k in ['schema','version','channel','git_sha','protocol_major']):raise ValueError('Platform identity mismatch')
 assets=[a for r in records for a in r['assets']]
 if len(assets)!=2 or {a['platform'] for a in assets}!={'linux-x86_64','windows-x86_64'}:raise ValueError('Platform set invalid')
 for asset in assets:
  if asset['name']!='tack-'+asset['platform']+'.zip' or not 0<asset['size']<=MAX_ARCHIVE or not re.fullmatch(r'[0-9a-f]{64}',asset['sha256']):raise ValueError('Invalid asset identity/size/hash')
  path=output/asset['name']
  if path.stat().st_size!=asset['size'] or digest(path)!=asset['sha256']:raise ValueError('Package checksum mismatch')
 manifest=dict(base,assets=assets)
 path=output/'update-manifest.json'
 if path.exists():raise ValueError('Manifest already exists')
 path.write_text(json.dumps(manifest,indent=2)+'\n')
 return manifest

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary-dir',type=Path);p.add_argument('--platform',choices=['linux-x86_64','windows-x86_64']);p.add_argument('--output',type=Path,required=True);p.add_argument('--channel',choices=['dev','stable'],default='dev');p.add_argument('--runner',nargs='*',default=[]);p.add_argument('--allow-dirty-dry-run',action='store_true');p.add_argument('--merge',type=Path,nargs='*');a=p.parse_args()
 if a.merge:result=merge(a.merge,a.output)
 else:
  if not a.binary_dir or not a.platform:p.error('--binary-dir and --platform required')
  result=package(a.binary_dir.resolve(),a.platform,a.output.resolve(),a.channel,a.runner,not a.allow_dirty_dry_run)
 print(json.dumps(result))
if __name__=='__main__':main()
