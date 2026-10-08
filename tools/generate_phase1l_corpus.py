#!/usr/bin/env python3
"""Phase 1L reusable fixtures. Run `plan` before `generate`; `cleanup` is guarded.

Only test_file/phase1l_generated is owned. No copies of existing images or bins.
JPEG determinism is scoped to the recorded Pillow version. Hardlinks deliberately
reuse payloads: object/path results are not evidence for 1000 unique photographs.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import random
import shutil
import stat
import struct
import subprocess
import time
import zlib


MIB = 1024 * 1024
RESERVE = 10 * 1024**3
BUDGET = 512 * MIB
SEED = 20261007
JPEG_QUALITY = 45
OWNER = "tack-phase1l-corpus-v1"
SENTINEL = ".phase1l-owned"
OWNERSHIP = "ownership.json"
JOURNAL = "ownership.next.json"
DEFAULT_ROOT = Path(__file__).resolve().parents[1] / "test_file/phase1l_generated"
AXES = (4096, 6000, 8192, 12000, 16000, 24000, 32000, 50000)
JPEG_HEIGHTS = (2304, 3500, 2048, 1024, 1024, 640, 512, 512)
PNG_HEIGHTS = (4096, 3000, 4096, 2048, 4096, 2048, 2048, 4096)
CROSSOVER_AXES = (1024, 2048, 3072)
MIXED_SIZES = ((1024, 768), (768, 1024), (1920, 1080), (1080, 1920),
               (2048, 2048), (4096, 2048), (2048, 4096), (6000, 3500))


def plan():
    """Estimates are admission reservations, not measured encoded sizes."""
    return {"schema": 1, "seed": SEED, "reserve_bytes": RESERVE,
            "budget_bytes": BUDGET, "estimated_encoded_bytes": 464750000 + 12 * MIB,
            "jpeg_quality": JPEG_QUALITY,
            "estimated_components_bytes": {"stress_1000_paths": 26250000,
                "stress_250_unique": 262500000, "mixed_prod_1000": 48000000,
                "huge_sweep": 128000000, "crossover": 12 * MIB},
            "stress_1000_paths": {"objects": 1000, "unique_payloads": 25,
                                  "dimensions": [6000, 3500]},
            "stress_250_unique": {"objects": 250, "unique_payloads": 250,
                                  "dimensions": [6000, 3500]},
            "mixed_prod_1000": {"objects": 1000, "unique_payloads": 96,
                                "dimensions": MIXED_SIZES},
            "huge_sweep": [{"jpeg": [axis, jh], "png": [axis, ph]}
                for axis, jh, ph in zip(AXES, JPEG_HEIGHTS, PNG_HEIGHTS)],
            "crossover": [[axis, axis] for axis in CROSSOVER_AXES],
            "limits": "512 MiB physical budget; >=10 GiB free; no copy fallback; "
                      "one <=64 MiB Pillow image; PNG streamed RGB rows; "
                      "JPEG grayscale where Pillow RGB storage would exceed 64 MiB; "
                      "estimates must be checked against receipts"}


def guarded_root(root):
    root = Path(os.path.abspath(root))
    if root.name != "phase1l_generated" or root.parent.name != "test_file":
        raise ValueError("output must be named test_file/phase1l_generated")
    for part in (root, *root.parents):
        if part.is_symlink():
            raise ValueError("symlink in output ancestry")
    if not root.parent.is_dir():
        raise ValueError("test_file parent must already exist")
    return root


def relative_path(value):
    path = Path(value)
    if not value or not path.parts or path.is_absolute() or any(p in (".", "..") for p in path.parts):
        raise ValueError("invalid ownership path")
    return path


def tree_stats(root):
    """Equivalent to du's inode deduplication; apparent includes every link."""
    seen, physical, apparent, files, links = set(), 0, 0, 0, 0
    for path in (root, *root.rglob("*")):
        info = path.lstat()
        key = (info.st_dev, info.st_ino)
        if stat.S_ISREG(info.st_mode):
            files += 1
            apparent += info.st_size
            links += int(key in seen)
        if key not in seen:
            physical += info.st_blocks * 512
            seen.add(key)
    return {"physical_du_bytes": physical, "apparent_bytes": apparent,
            "regular_file_paths": files, "repeated_hardlink_paths": links}


class OwnedTree:
    def __init__(self, root, create=False):
        self.root = guarded_root(root)
        if create:
            if self.root.exists():
                raise ValueError("generated root already exists; inspect or clean it explicitly")
            if shutil.disk_usage(self.root.parent).free < RESERVE + BUDGET:
                raise ValueError("generation needs 512 MiB reservation plus 10 GiB free")
            self.root.mkdir()
            (self.root / SENTINEL).write_text(OWNER + "\n", encoding="utf-8")
            self.state = {"owner": OWNER, "schema": 1, "seed": SEED,
                          "status": "generating", "files": [SENTINEL, OWNERSHIP, JOURNAL],
                          "directories": []}
            self.save()
        else:
            if (self.root / SENTINEL).is_symlink() or (self.root / OWNERSHIP).is_symlink():
                raise ValueError("ownership metadata must be regular files")
            if (self.root / SENTINEL).read_text(encoding="utf-8") != OWNER + "\n":
                raise ValueError("missing or invalid ownership sentinel")
            self.state = json.loads((self.root / OWNERSHIP).read_text(encoding="utf-8"))
            if self.state.get("owner") != OWNER or self.state.get("schema") != 1:
                raise ValueError("unsupported ownership manifest")
            self.validate()

    def save(self):
        journal, target = self.root / JOURNAL, self.root / OWNERSHIP
        if journal.is_symlink() or target.is_symlink():
            raise ValueError("symlink in ownership metadata")
        journal.write_text(json.dumps(self.state, indent=2) + "\n", encoding="utf-8")
        journal.replace(target)

    def validate(self):
        files, dirs = self.state["files"], self.state["directories"]
        if not {SENTINEL, OWNERSHIP}.issubset(files) or len(set(files + dirs)) != len(files + dirs):
            raise ValueError("invalid or duplicate ownership entries")
        for name in files + dirs:
            path = self.root / relative_path(name)
            for ancestor in (path, *path.parents):
                if ancestor == self.root:
                    break
                if ancestor.is_symlink():
                    raise ValueError("symlink in owned tree")
            if path.exists() and (path.is_dir() != (name in dirs) or
                                  not (path.is_dir() or path.is_file())):
                raise ValueError("owned entry changed type")
        actual = {p.relative_to(self.root).as_posix() for p in self.root.rglob("*")}
        if actual - set(files + dirs):
            raise ValueError("unowned entries in generated tree; refusing cleanup or writes")

    def register(self, name, directory=False):
        path = self.root / relative_path(name)
        if path.exists() or path.is_symlink():
            raise ValueError("refusing to overwrite existing output")
        key = "directories" if directory else "files"
        if name in self.state[key]:
            raise ValueError("output already registered")
        self.state[key].append(name)
        self.save()  # Journal first, so interrupted generation remains removable.
        if directory:
            path.mkdir()
        return path

    def admit(self, extra=0):
        if tree_stats(self.root)["physical_du_bytes"] + extra > BUDGET:
            raise ValueError("512 MiB generated-data budget exhausted")
        if shutil.disk_usage(self.root).free < RESERVE + extra:
            raise ValueError("10 GiB disk reserve would be crossed")

    def write_json(self, name, value):
        self.admit(64 * 1024)
        self.register(name).write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")

    def cleanup(self):
        self.validate()  # Preflight all entries before unlinking any of them.
        for name in self.state["files"]:
            if name not in (SENTINEL, OWNERSHIP):
                (self.root / name).unlink(missing_ok=True)
        for name in sorted(self.state["directories"], key=lambda p: len(Path(p).parts), reverse=True):
            path = self.root / name
            if path.exists():
                path.rmdir()
        (self.root / OWNERSHIP).unlink()
        (self.root / SENTINEL).unlink()
        self.root.rmdir()


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(MIB), b""):
            digest.update(block)
    return digest.hexdigest()


class CheckedOutput:
    """Expose writes, not fileno: Pillow must not bypass disk admission checks."""
    def __init__(self, stream, check):
        self.stream, self.check = stream, check

    def write(self, data):
        self.check(len(data) + 4096)
        return self.stream.write(data)

    def flush(self):
        self.stream.flush()


def jpeg(path, width, height, seed, check=lambda extra: None, quality=JPEG_QUALITY):
    from PIL import Image, ImageDraw
    # Pillow's RGB ImagingCore stores four bytes per pixel, not packed RGB3.
    mode = "L" if width * height * 4 > 64 * MIB else "RGB"
    channels, storage_channels = (1, 1) if mode == "L" else (3, 4)
    if width * height * storage_channels > 64 * MIB:
        raise ValueError("JPEG working image exceeds 64 MiB")
    rng = random.Random(seed)
    # Smooth fields plus small, crisp technical detail keep compression useful
    # without constructing random 21-megapixel payloads or requiring numpy.
    with Image.frombytes(mode, (192, 128), rng.randbytes(192 * 128 * channels)) as field:
        image = field.resize((width, height), Image.Resampling.BILINEAR)
    try:
        draw = ImageDraw.Draw(image)
        for index in range(350):
            x, y = rng.randrange(width), rng.randrange(height)
            color = rng.randrange(256) if mode == "L" else tuple(rng.randrange(256) for _ in range(3))
            draw.rectangle((x, y, min(width - 1, x + rng.randrange(4, 100)),
                            min(height - 1, y + rng.randrange(4, 80))), outline=color, width=2)
            draw.text((x, y), f"{seed:x}/{index}", fill=color)
        with path.open("xb", buffering=0) as stream:
            image.save(CheckedOutput(stream, check), "JPEG", quality=quality,
                       subsampling=2, optimize=False, progressive=False)
    finally:
        image.close()
    return mode


def procedural_row(width, y, seed):
    """Source-detail tile rows: reproducible noise, coordinate marks and contours.

    Repeated 256px detail compresses well; 32px boundaries encode absolute tile
    coordinates, making locally sampled regions recognizable and comparable.
    """
    tile = random.Random(seed + y % 256).randbytes(256 * 3)
    row = bytearray((tile * math.ceil(width / 256))[:width * 3])
    for x in range(0, width, 32):
        color = bytes(((x // 32) % 256, (y // 32) % 256,
                       ((x // 32) // 256) ^ ((y // 32) // 256)))
        span = min(4 if y % 32 else 16, width - x)
        row[x * 3:(x + span) * 3] = color * span
    return bytes(row)


def png_chunk(stream, kind, data):
    stream.write(struct.pack(">I", len(data)) + kind + data +
                 struct.pack(">I", zlib.crc32(kind + data) & 0xffffffff))


def png(path, width, height, seed, check=lambda extra=0: None):
    if not (1 <= width <= 50000 and 1 <= height <= 4096):
        raise ValueError("PNG dimensions exceed controlled fixture bounds")
    with path.open("xb", buffering=0) as stream:
        stream.write(b"\x89PNG\r\n\x1a\n")
        png_chunk(stream, b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
        compressor = zlib.compressobj(6)
        for y in range(height):
            data = compressor.compress(b"\0" + procedural_row(width, y, seed))
            if data:
                check(len(data) + 4096)
                png_chunk(stream, b"IDAT", data)
        data = compressor.flush()
        check(len(data) + 4096)
        png_chunk(stream, b"IDAT", data)
        png_chunk(stream, b"IEND", b"")


def bank_layout(index, width, height, seed):
    # Asymmetric topical clusters with staggered shelves, gaps and varied scale.
    rng = random.Random(seed + index)
    group, local = divmod(index, 40)
    scale = rng.uniform(0.075, 0.15)
    display_width, display_height = width * scale, height * scale
    return {"x": (group % 5) * 4600 + (local % 8) * 530 + rng.randrange(-70, 71),
            "y": (group // 5) * 3400 + (local // 8) * 570 + rng.randrange(-90, 91),
            "width": round(display_width, 3), "height": round(display_height, 3)}


def manifest(objects, sources, elapsed, limitation):
    return {"schema": 1, "seed": SEED, "objects": objects, "sources": sources,
            "source_count": len(sources), "generation_seconds": elapsed,
            "average_encoded_bytes": sum(s["encoded_bytes"] for s in sources) / len(sources),
            "dimensions": sorted({(s["width"], s["height"]) for s in sources}),
            "limitation": limitation}


def generate_stress(tree, name, count, source_count, sizes, seed):
    started, sources, objects = time.monotonic(), [], []
    tree.register(name, directory=True)
    for index in range(count):
        path = tree.register(f"{name}/image-{index:04d}.jpg")
        if index < source_count:
            tree.admit(4 * MIB)
            width, height = sizes[index % len(sizes)]
            mode = jpeg(path, width, height, seed + index, tree.admit)
            tree.admit()
            sources.append({"path": path.relative_to(tree.root).as_posix(),
                "sha256": sha256(path), "width": width, "height": height,
                "encoded_bytes": path.stat().st_size, "seed": seed + index,
                "mode": mode, "jpeg_quality": JPEG_QUALITY})
        else:
            os.link(tree.root / sources[index % source_count]["path"], path)
        source = sources[index % source_count]
        geometry = bank_layout(index, source["width"], source["height"], seed)
        objects.append({"id": index, "path": path.relative_to(tree.root).as_posix(),
            "source_sha256": source["sha256"], "source_width": source["width"],
            "source_height": source["height"], **geometry})
    note = (f"{count} distinct paths; {source_count} unique JPEG payloads; hardlinks "
            "reuse inodes and OS caches. Procedural technical references, not photographs.")
    result = manifest(objects, sources, time.monotonic() - started, note)
    result["filesystem"] = tree_stats(tree.root / name)
    tree.write_json(f"{name}.json", result)
    return result


def generate_huge(tree):
    started, sources, objects = time.monotonic(), [], []
    tree.register("huge_sweep", directory=True)
    for index, axis in enumerate(AXES):
        for extension, height in (("jpg", JPEG_HEIGHTS[index]), ("png", PNG_HEIGHTS[index])):
            seed = SEED + 10000 + index
            path = tree.register(f"huge_sweep/{axis}x{height}.{extension}")
            tree.admit(8 * MIB)
            if extension == "jpg":
                mode = jpeg(path, axis, height, seed, tree.admit)
            else:
                png(path, axis, height, seed, tree.admit)
                mode = "RGB"
            tree.admit()
            source = {"path": path.relative_to(tree.root).as_posix(), "sha256": sha256(path),
                      "width": axis, "height": height, "encoded_bytes": path.stat().st_size,
                      "seed": seed, "format": extension, "mode": mode,
                      "jpeg_quality": JPEG_QUALITY if extension == "jpg" else None}
            sources.append(source)
            objects.append({"id": len(objects), "path": source["path"],
                "source_sha256": source["sha256"], "source_width": axis,
                "source_height": height, "x": index * 1800, "y": 0 if extension == "jpg" else 1600,
                "width": 1500, "height": 1500 * height / axis})
    result = manifest(objects, sources, time.monotonic() - started,
        "Controlled axis sweep, bounded heights; not square-image cost evidence. "
        "PNG RGB8 static/noninterlaced streamed procedural detail; 50k JPEG grayscale. "
        "Old 6000-axis baseline rejection is an expected result, not a skipped case.")
    result["filesystem"] = tree_stats(tree.root / "huge_sweep")
    tree.write_json("huge_sweep.json", result)
    return result


def axis_names():
    return tuple(f"axis-{axis}-{format_name}" for axis in AXES
                 for format_name in ("jpeg", "png"))


def extra_huge_manifests(huge):
    """Single-source cost evidence and a codec-homogeneous multi-image case.

    Sources are reused by path: these manifests allocate no image payloads.
    Independent origin placement gives every axis case the same initial camera.
    """
    result = {}
    for obj in huge["objects"]:
        format_name = "png" if Path(obj["path"]).suffix == ".png" else "jpeg"
        name = f"axis-{obj['source_width']}-{format_name}"
        result[name] = {"schema": 1, "objects": [dict(obj, id=0, x=0, y=0)],
            "limitation": "One generated source, reused without copying; bounded "
                          "height, not square-image cost evidence."}
    if set(result) != set(axis_names()):
        raise ValueError("axis source matrix is incomplete or unexpected")
    png_objects = [obj for obj in huge["objects"]
                   if Path(obj["path"]).suffix == ".png"][-3:]
    objects, y = [], 0
    for index, obj in enumerate(png_objects):
        height = 1500 * obj["source_height"] / obj["source_width"]
        objects.append(dict(obj, id=index, x=0, y=y, width=1500, height=height))
        y += height + 100
    result["several_huge_png"] = {"schema": 1, "objects": objects,
        "limitation": "Three static RGB8 noninterlaced PNG sources: 24k, 32k, "
                      "50k axes; vertically separated bands, no overlap or "
                      "payload copies. Overview fits all; deep zoom varies "
                      "visible sources and therefore is not simultaneous "
                      "full-resolution evidence."}
    result["overlapped_huge_png"] = {"schema": 1,
        "objects": [dict(obj, y=index * 16) for index, obj in enumerate(objects)],
        "limitation": "Reuses the same 24k, 32k and 50k PNG sources. Three "
                      "bands at x=0, y=0/16/32, width=1500 deliberately overlap "
                      "to demand all sources at deep zoom. Occlusion and "
                      "actual simultaneous tile admission must be recorded; "
                      "geometry alone is not performance evidence."}
    return result


def write_extra_huge_manifests(tree, huge):
    manifests = extra_huge_manifests(huge)
    for name, value in manifests.items():
        tree.write_json(name + ".json", value)
    return tuple(manifests)


def mixed_huge_visible_manifest(huge, mixed):
    """Compact ordinary references share the 50k PNG's deep-zoom viewport."""
    giant = next(obj for obj in huge["objects"]
                 if obj["source_width"] == 50000 and Path(obj["path"]).suffix == ".png")
    objects = [dict(giant, id=0, x=0, y=0, width=1500,
                    height=1500 * giant["source_height"] / giant["source_width"])]
    placements = ((730, 44), (756, 50), (728, 80), (768, 20))
    for index, ((x, y), source) in enumerate(zip(placements, mixed["objects"][:4]), 1):
        objects.append(dict(source, id=index, x=x, y=y, width=35,
                            height=35 * source["source_height"] / source["source_width"]))
    if len(objects) != 5:
        raise ValueError("mixed visible case requires four ordinary sources")
    return {"schema": 1, "objects": objects,
        "limitation": "Reuses one 50k PNG and four ordinary JPEG sources with "
                      "compact placements near (750,64). Actual visible source "
                      "counts and tile admission must be measured."}


def crossover_names():
    return tuple(f"crossover-{axis}-png" for axis in CROSSOVER_AXES)


def generate_crossover(tree):
    """Stream tiny threshold probes; total encoded payload never exceeds 12 MiB."""
    tree.register("crossover", directory=True)
    used = 0
    for axis, name in zip(CROSSOVER_AXES, crossover_names()):
        seed = SEED + 20000 + axis
        path = tree.register(f"crossover/{axis}x{axis}.png")

        def admit(extra):
            current = path.stat().st_size if path.exists() else 0
            if used + current + extra > 12 * MIB:
                raise ValueError("crossover PNG encoded-data budget exhausted")
            tree.admit(extra)

        png(path, axis, axis, seed, admit)
        used += path.stat().st_size
        obj = {"id": 0, "path": path.relative_to(tree.root).as_posix(),
               "source_sha256": sha256(path), "source_width": axis,
               "source_height": axis, "x": 0, "y": 0,
               "width": 1500, "height": 1500}
        tree.write_json(name + ".json", {"schema": 1, "objects": [obj],
            "encoded_bytes": path.stat().st_size, "seed": seed,
            "limitation": "Square RGB8 static noninterlaced procedural PNG; "
                          "streamed rows without a full source frame. "
                          "Synthetic decoder-policy crossover probe."})
    return crossover_names()


def generate(root):
    from PIL import __version__ as pillow_version
    started = time.monotonic()
    tree = OwnedTree(root, create=True)
    tree.register(".gitignore").write_text("*\n", encoding="utf-8")
    tree.write_json("plan.json", plan())
    results = [generate_stress(tree, "stress_1000_paths", 1000, 25, [(6000, 3500)], SEED),
        generate_stress(tree, "stress_250_unique", 250, 250, [(6000, 3500)], SEED + 1000),
        generate_stress(tree, "mixed_prod_1000", 1000, 96, MIXED_SIZES, SEED + 2000),
        generate_huge(tree)]
    huge, mixed = results[3]["objects"], results[2]["objects"]
    scenarios = {"single_huge": [huge[-1]], "several_huge": huge[-6:],
                 "huge_plus_ordinary": [huge[-1]] + mixed[:100]}
    for name, entries in scenarios.items():
        objects = [dict(o, id=i) for i, o in enumerate(entries)]
        tree.write_json(name + ".json", {"schema": 1, "objects": objects,
            "limitation": "Reuses generated sources; no additional corpus copies."})
    write_extra_huge_manifests(tree, results[3])
    tree.write_json("mixed_huge_visible.json", mixed_huge_visible_manifest(results[3], results[2]))
    generate_crossover(tree)
    tree.state.update(status="complete", pillow_version=pillow_version,
                      jpeg_quality=JPEG_QUALITY,
                      zlib_version=zlib.ZLIB_RUNTIME_VERSION,
                      jpeg_mode_policy="grayscale if Pillow RGB4 storage would exceed 64 MiB",
                      generation_seconds=time.monotonic() - started)
    tree.save()
    tree.admit()
    # Ownership metadata is excluded from its own receipt to avoid self-size recursion.
    receipt = {**tree_stats(tree.root), "generation_seconds": tree.state["generation_seconds"],
               "pillow_version": pillow_version, "zlib_version": zlib.ZLIB_RUNTIME_VERSION,
               "jpeg_quality": JPEG_QUALITY,
               "jpeg_mode_policy": tree.state["jpeg_mode_policy"], "plan": plan(),
               "measurement": "du-style st_blocks*512, deduplicated device/inode; "
                              "sampled before receipt write; verify with du -s -B1"}
    tree.write_json("receipt.json", receipt)
    return receipt


def build_boards(tree, binary, names, metadata_only=False):
    """Build only named, new outputs; existing manifests/boards stay untouched."""
    results = []
    for name in names:
        tree.admit(16 * MIB)
        board = tree.register(f"boards/{name}.tack")
        report = tree.register(f"boards/{name}.json")
        tree.register(f"boards/{name}.tack.tack-lock")
        source_manifest = tree.root / (name + ".json")
        command = ([str(binary), str(source_manifest), str(board), str(report)] if metadata_only else
                   [str(binary), "create", str(board), "--linked", "--manifest",
                    str(source_manifest), "--output", str(report)])
        completed = subprocess.run(command, check=False, capture_output=True, text=True)
        results.append({"name": name, "returncode": completed.returncode,
                        "stderr": completed.stderr[-4000:]})
        tree.admit()
    return results


def boards(root, binary, builder=None):
    tree = OwnedTree(root)
    if tree.state["status"] != "complete":
        raise ValueError("corpus is incomplete")
    binary = Path(builder or binary).resolve(strict=True)
    tree.register("boards", directory=True)
    names = ("stress_1000_paths", "stress_250_unique", "mixed_prod_1000", "huge_sweep",
             "single_huge", "several_huge", "huge_plus_ordinary",
             "several_huge_png", "overlapped_huge_png", "mixed_huge_visible") + axis_names() + crossover_names()
    results = build_boards(tree, binary, names, bool(builder))
    tree.write_json("board-results.json", {"binary": str(binary), "sha256": sha256(binary),
        "metadata_only_builder": bool(builder), "results": results,
        "limitation": "Baseline huge-image rejection is expected; "
        "exit status alone is not performance evidence. Reuses the existing binary."})
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "generate", "boards", "cleanup"))
    parser.add_argument("--output", type=Path, default=DEFAULT_ROOT)
    parser.add_argument("--binary", type=Path, default=DEFAULT_ROOT.parents[1] / "bin/tack")
    parser.add_argument("--builder", type=Path, help="existing phase1l_corpus example binary; "
                        "linked metadata-only boards without overview preparation")
    args = parser.parse_args()
    try:
        if args.command == "plan":
            result = plan()
        elif args.command == "generate":
            result = generate(args.output)
        elif args.command == "boards":
            result = boards(args.output, args.binary, args.builder)
        else:
            OwnedTree(args.output).cleanup()
            result = {"removed": str(args.output)}
    except (ValueError, OSError, KeyError, TypeError) as error:
        parser.exit(1, f"phase1l corpus refused: {error}\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
