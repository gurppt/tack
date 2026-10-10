#!/usr/bin/env python3
"""Build pinned, unmodified libjpeg-turbo locally; never installs system packages."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import urllib.request

VERSION = "3.2.0"
ARCHIVE_SHA256 = "6f30092cef9fb839779646608f4ee14ae3cbac989c47fa05e841b0841f09878e"
URL = f"https://github.com/libjpeg-turbo/libjpeg-turbo/releases/download/{VERSION}/libjpeg-turbo-{VERSION}.tar.gz"


def library_alias(prefix):
    # turbojpeg-sys's explicit lookup requests turbojpeg.lib on MSVC, while
    # upstream CMake names its static target turbojpeg-static.lib.
    original = prefix / "lib/turbojpeg-static.lib"
    if original.exists():
        shutil.copy2(original, prefix / "lib/turbojpeg.lib")


def prepare(root, target=None):
    root = root.resolve()
    prefix = root / (f"libjpeg-turbo-{VERSION}-windows-gnu" if target else f"libjpeg-turbo-{VERSION}")
    record = prefix / "native-build.json"
    if record.exists():
        previous = json.loads(record.read_text())
        library_alias(prefix)
        libraries = list((prefix / "lib").glob("*turbojpeg*.a")) + list((prefix / "lib").glob("*turbojpeg*.lib"))
        decoder = prefix / ("bin/djpeg.exe" if target or __import__("os").name == "nt" else "bin/djpeg")
        if previous.get("source_sha256") == ARCHIVE_SHA256 and previous.get("target") == target and libraries and (prefix / "include/turbojpeg.h").is_file() and decoder.is_file():
            previous["decoder_sha256"] = hashlib.sha256(decoder.read_bytes()).hexdigest()
            previous["library_sha256"] = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in libraries}
            record.write_text(json.dumps(previous, indent=2) + "\n")
            print(prefix)
            return
    if not shutil.which("cmake") or not shutil.which("nasm"):
        raise RuntimeError("CMake, a C compiler and NASM on PATH are required (no privileged installation is performed)")
    root.mkdir(parents=True, exist_ok=True)
    archive = root / f"libjpeg-turbo-{VERSION}.tar.gz"
    if not archive.exists():
        with urllib.request.urlopen(URL, timeout=60) as response, archive.open("wb") as archive_output:
            shutil.copyfileobj(response, archive_output)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != ARCHIVE_SHA256:
        raise RuntimeError("libjpeg-turbo source SHA256 mismatch")
    source = root / "source" / f"libjpeg-turbo-{VERSION}"
    if not source.exists():
        with tarfile.open(archive) as package:
            package.extractall(root / "source", filter="data")
    build = root / ("turbojpeg-build-windows-gnu" if target else "turbojpeg-build")
    configure = ["cmake", "-S", str(source), "-B", str(build),
                 f"-DCMAKE_INSTALL_PREFIX={prefix}", "-DCMAKE_INSTALL_LIBDIR=lib",
                 "-DCMAKE_BUILD_TYPE=Release", "-DENABLE_SHARED=OFF", "-DENABLE_STATIC=ON",
                 "-DWITH_TURBOJPEG=ON", "-DREQUIRE_SIMD=ON", "-DWITH_JAVA=OFF", "-DWITH_CRT_DLL=ON"]
    if target:
        compiler = shutil.which("x86_64-w64-mingw32-gcc")
        resource = shutil.which("x86_64-w64-mingw32-windres")
        if not compiler or not resource:
            raise RuntimeError("MinGW x64 GCC and windres are required")
        toolchain = root / "windows-gnu-toolchain.cmake"
        toolchain.write_text(f'set(CMAKE_SYSTEM_NAME Windows)\nset(CMAKE_SYSTEM_PROCESSOR x86_64)\nset(CMAKE_C_COMPILER "{compiler}")\nset(CMAKE_RC_COMPILER "{resource}")\n')
        configure += [f"-DCMAKE_TOOLCHAIN_FILE={toolchain}", "-DWITH_TESTS=OFF", "-DWITH_SIMD=ON", f"-DCMAKE_ASM_NASM_COMPILER={shutil.which("nasm")}"]
    subprocess.run(configure, check=True)
    subprocess.run(["cmake", "--build", str(build), "--config", "Release", "--parallel", "2"], check=True)
    subprocess.run(["cmake", "--install", str(build), "--config", "Release"], check=True)
    library_alias(prefix)
    notices = prefix / "notices"
    notices.mkdir(exist_ok=True)
    for filename in ["LICENSE.md", "README.ijg"]:
        shutil.copy2(source / filename, notices / filename)
    # Pure build metadata, never read on the application's frame path.
    record.write_text(json.dumps(dict(version=VERSION, target=target, source_url=URL,
        source_sha256=ARCHIVE_SHA256, cmake_command=configure,
        decoder_sha256=hashlib.sha256(next((prefix / "bin").glob("djpeg*")).read_bytes()).hexdigest(),
        library_sha256={p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in (prefix / "lib").iterdir() if "turbojpeg" in p.name},
        compiler_cache=[line for line in (build / "CMakeCache.txt").read_text().splitlines() if line.startswith("CMAKE_C_COMPILER:")],
        nasm=subprocess.run(["nasm", "-v"], check=True, capture_output=True, text=True).stdout.strip()), indent=2) + "\n")
    print(prefix)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("target/native"))
    parser.add_argument("--target", choices=["windows-gnu"], default=None)
    args = parser.parse_args()
    prepare(args.root, args.target)
