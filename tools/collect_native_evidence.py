#!/usr/bin/env python3
"""Compact Mission 0.6 raw evidence without discarding censored demand metrics."""
import hashlib
import json
from pathlib import Path
import zipfile

ROOT = Path("benchmark-results")
OUT = Path("benchmarks")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(name, value):
    (OUT / ("mission0_6-" + name + ".json")).write_text(json.dumps(value, indent=2) + "\n")


def suite(name):
    root = ROOT / ("mission0_6-" + name)
    environment = json.loads((root / "environment.json").read_text())
    assert digest(root / "tack-app") == environment["binary_sha256"]
    rows = json.loads((root / "summary.json").read_text())
    artifacts = {str(p): digest(p) for p in [root / "environment.json", root / "summary.json", root / "source-snapshot.zip"]}
    for row in rows:
        raw = root / (row["scenario"] + ".json")
        artifacts[str(raw)] = digest(raw)
        data = json.loads(raw.read_text())
        assert data["worker_profile_dropped"] == 0 and data["coverage_episodes_dropped"] == 0
        assert data["final"]["decode_errors"] == 0
        assert data["workers"] == 2 and data["prefetch"] == "none" and not data["headless"]
        assert data["thumbnail_decoder"] == row["thumbnail_decoder"]
    return dict(environment=environment, traces=rows, artifacts_sha256=artifacts)


def main():
    for mode in ["baseline", "candidate"]:
        write(mode, dict(schema=1, cold=suite(mode), repeat=suite("repeat-" + mode), warm=suite("warm-" + mode)))
    warm = ROOT / "mission0_6-warm-input/provenance.json"
    provenance = json.loads(warm.read_text())
    write("warm-input", {k: v for k, v in provenance.items() if k != "entries"} | dict(raw=str(warm), sha256=digest(warm)))
    experiments = {}
    for label in ["micro", "photo", "progressive"]:
        path = ROOT / ("mission0_6-" + label) / "summary.json"
        data = json.loads(path.read_text())
        for variant in data["variants"].values():
            rows = variant.pop("rows")
            variant["output_layouts"] = [json.loads(s) for s in sorted({json.dumps({k: r[k] for k in ["decoded_dimensions", "decoded_bytes", "thumbnail_dimensions", "thumbnail_bytes"]}, sort_keys=True) for r in rows})]
            variant["encoded_bytes_range"] = [min(r["encoded_bytes"] for r in rows), max(r["encoded_bytes"] for r in rows)]
        data["raw"] = str(path)
        data["raw_sha256"] = digest(path)
        experiments[label] = data
    write("micro", dict(schema=1, experiments=experiments))
    idle = ROOT / "mission0_6-idle/summary.json"
    if idle.exists():
        write("idle", dict(schema=1, raw=str(idle), raw_sha256=digest(idle), **json.loads(idle.read_text())))
    # Runtime files in the measured candidate must still match the final checkout.
    # Subsequent documentation/test-only changes do not retroactively alter its ZIP.
    with zipfile.ZipFile(ROOT / "mission0_6-candidate/source-snapshot.zip") as archive:
        runtime = [name for name in archive.namelist() if "/src/" in name or name in ["Cargo.toml", "Cargo.lock", ".cargo/config.toml"] or (name.startswith("crates/") and name.endswith("Cargo.toml"))]
        for name in runtime:
            assert archive.read(name) == Path(name).read_bytes(), "Measured runtime changed: " + name
    print("20 native trace reports and measured runtime provenance collected")


if __name__ == "__main__":
    main()
