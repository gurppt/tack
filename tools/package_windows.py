#!/usr/bin/env python3
"""Package the explicit GNU Windows build with pinned SIMD decoder and UI assets."""
import datetime, hashlib, json, shutil, subprocess, zipfile
from pathlib import Path
from install_toolbar_icons import install_missing

def digest(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for b in iter(lambda:f.read(131072),b''):h.update(b)
    return h.hexdigest()

def main():
    root=Path(__file__).resolve().parents[1]
    built=root/'target/x86_64-pc-windows-gnu/release'
    native=root/'target/native/libjpeg-turbo-3.2.0-windows-gnu'
    package=root/'bin/tack-windows-x86_64';package.mkdir(parents=True,exist_ok=True)
    record=json.loads((native/'native-build.json').read_text())
    if record.get('target')!='windows-gnu' or not record.get('nasm'):
        raise RuntimeError('Windows SIMD provenance missing')
    symbols=subprocess.check_output(['x86_64-w64-mingw32-nm',str(native/'lib/libjpeg.a')],text=True)
    if not all(' T jsimd_' in symbols and suffix in symbols for suffix in ['_sse2','_avx2']):
        raise RuntimeError('Windows SIMD objects missing')
    for source,name in [(built/'tack-app.exe','tack.exe'),(built/'tack-server.exe','tack-server.exe'),(native/'bin/djpeg.exe','tack-jpeg-decoder.exe')]:
        shutil.copy2(source,package/name)
        subprocess.run(['x86_64-w64-mingw32-strip','--strip-all',str(package/name)],check=True)
    for p in built.glob('tack-icon*'):shutil.copy2(p,package/p.name)
    shutil.copy2(built/'tack-about.png',package/'tack-about.png')
    shutil.copy2(built/'tack-about-logo.png',package/'tack-about-logo.png')
    install_missing(root/'gfx/icons', package/'gfx/icons')
    imports={}
    for p in package.glob('*.exe'):
        data=subprocess.check_output(['x86_64-w64-mingw32-objdump','-p',str(p)],text=True)
        imports[p.name]=[line.split(':',1)[1].strip() for line in data.splitlines() if 'DLL Name:' in line]
        if any(name.lower().startswith(('libgcc','libstdc++','libwinpthread','turbojpeg')) for name in imports[p.name]):
            raise RuntimeError('Unexpected external runtime DLL')
    notices=package/'LICENSES';notices.mkdir(exist_ok=True)
    for p in (native/'notices').iterdir():
        if p.is_file():shutil.copy2(p,notices/p.name)
    shutil.copy2(native/'native-build.json',notices/'native-build.json')
    (package/'README.txt').write_text('Tack — Windows x64\nRun tack.exe. Keep this folder together (server, SIMD JPEG decoder, icons and About).\nTrusted LAN sharing only. Application license/contact to be defined.\nCross-built on Linux; physical Windows desktop acceptance pending.\n',encoding='utf8')
    (package/'LISEZ-MOI.txt').write_text('Tack — Windows x64\nLancer tack.exe et conserver le dossier complet.\nDécodeur JPEG libjpeg-turbo 3.2.0 avec SIMD SSE2/AVX2 intégré.\nLicence et contact Tack à définir. Test desktop Windows physique en attente.\n',encoding='utf8')
    (package/'BUILD.json').write_text(json.dumps(dict(
        commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),
        built_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),target='x86_64-pc-windows-gnu',
        working_tree=True, jpeg='libjpeg-turbo 3.2.0 static SIMD SSE2/AVX2',native=record,
        executable_imports=imports,files={p.name:dict(bytes=p.stat().st_size,sha256=digest(p)) for p in package.glob('*.exe')}
    ),indent=2)+'\n',encoding='utf8')
    archive=root/'bin/tack-windows-x86_64.zip'
    with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
        for p in sorted(package.rglob('*')):
            if p.is_file():z.write(p,p.relative_to(package.parent))
    print(json.dumps(dict(path=str(archive),bytes=archive.stat().st_size,sha256=digest(archive))))
if __name__=='__main__':main()
