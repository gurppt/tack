#!/usr/bin/env python3
"""Isolated Pillow/libjpeg reference experiments, never the application loader.

These measurements are not an attribution of Rust/zune/jpeg-decoder timings.
All inputs are corpus sources; output goes under the ignored benchmark directory.
"""
import argparse
import hashlib
import io
import json
import math
from pathlib import Path
import platform
import time
from PIL import Image, ImageChops, ImageStat, features, __version__


def timed(operation):
    start = time.perf_counter()
    result = operation()
    return result, (time.perf_counter() - start) * 1000


def run(corpus, output):
    output.mkdir(parents=True, exist_ok=False)
    rows = []
    formats = []
    tiles = []
    for source in sorted((corpus / "sources").glob("*.jpg"))[:8]:
        encoded, read_ms = timed(source.read_bytes)
        for edge in [64, 96, 128, 512, 2048]:
            representations = {}
            for scaled in [False, True]:
                image, header_ms = timed(lambda: Image.open(io.BytesIO(encoded)))
                if scaled:
                    image.draft("RGB", (edge, edge * 3 // 4))
                dimensions = image.size
                _, decode_ms = timed(image.load)
                thumb, resize_ms = timed(lambda: image.resize((edge, edge * 3 // 4), Image.Resampling.BOX))
                representations[scaled] = thumb
                rows.append(dict(source=source.name, edge=edge, scaled=scaled,
                                 source_read_ms=read_ms, header_ms=header_ms,
                                 decode_ms=decode_ms, resize_ms=resize_ms,
                                 decoded_rgb_bytes=math.prod(dimensions) * 3))
                image.close()
            difference = ImageStat.Stat(ImageChops.difference(representations[False], representations[True]))
            mse = sum(v * v for v in difference.rms) / 3
            rows[-1]["psnr_against_full_box_db"] = 10 * math.log10(255**2/mse) if mse else None
            thumb = representations[True]
            for fmt in ["PNG", "JPEG", "raw-rgba"]:
                def encode():
                    if fmt == "raw-rgba":
                        return thumb.convert("RGBA").tobytes()
                    buffer = io.BytesIO()
                    thumb.save(buffer, fmt, **({"quality": 90} if fmt == "JPEG" else {}))
                    return buffer.getvalue()
                payload, encode_ms = timed(encode)
                def decode():
                    if fmt == "raw-rgba":
                        return Image.frombytes("RGBA", thumb.size, payload)
                    cached = Image.open(io.BytesIO(payload)); cached.load(); return cached
                decoded, decode_ms = timed(decode)
                formats.append(dict(source=source.name, edge=edge, format=fmt,
                                    encode_ms=encode_ms, decode_ms=decode_ms, bytes=len(payload)))
                decoded.close()
            if source.name.endswith("0000.jpg") and edge in [64, 96, 128]:
                thumb.resize((512,384), Image.Resampling.NEAREST).save(output / f"thumbnail-{edge}.png")
            if edge == 2048:
                for size in [256,512]:
                    cropped = thumb.crop((512,512,512+size,512+size))
                    buf = io.BytesIO(); _, encode_ms = timed(lambda: cropped.save(buf, "PNG"))
                    payload = buf.getvalue()
                    def decode_tile():
                        tile = Image.open(io.BytesIO(payload)); tile.load(); return tile
                    tile, decode_ms = timed(decode_tile)
                    tiles.append(dict(source=source.name, tile_size=size, bytes=len(payload), encode_ms=encode_ms,
                                      decode_ms=decode_ms, rgba_bytes=size*size*4,
                                      files_per_2048_lod=math.ceil(2048/size)*math.ceil(1536/size),
                                      files_per_original=math.ceil(6000/size)*math.ceil(4500/size)))
                    tile.close(); cropped.close()
            for representation in representations.values():
                representation.close()
    result = dict(schema=1, platform=platform.platform(), pillow=__version__,
                  libjpeg=features.version("jpg"), libjpeg_turbo=features.check_feature("libjpeg_turbo"),
                  harness_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  limitation="Separate native Pillow microbenchmark, not the Rust application. Cached tile timings exclude initial full-source decode and tile generation. PSNR on synthetic content does not establish artist-rated usefulness.",
                  decode=rows, formats=formats, tiles=tiles)
    (output / "experiment.json").write_text(json.dumps(result,indent=2)+"\n")
    return result


if __name__ == "__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus",type=Path,default=Path("benchmark-data/mission0"))
    parser.add_argument("--output",type=Path,required=True)
    args=parser.parse_args()
    run(args.corpus,args.output)
