"""Narrow deterministic fixture and ownership checks; no stress corpus or GPU."""
import hashlib
import json
import os
from pathlib import Path
import struct
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

from PIL import Image

from generate_phase1l_corpus import (AXES, BUDGET, JPEG_HEIGHTS, PNG_HEIGHTS, OWNERSHIP,
    OWNER, OwnedTree, RESERVE, SENTINEL, generate_stress, guarded_root, jpeg,
    plan, png, procedural_row, relative_path, tree_stats, axis_names,
    extra_huge_manifests, write_extra_huge_manifests, generate_crossover,
    mixed_huge_visible_manifest)


class CorpusTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.parent = Path(self.temp.name) / "test_file"
        self.parent.mkdir()
        self.root = self.parent / "phase1l_generated"
        # The tiny tests are valid even on hosts with less than the real reserve.
        self.disk = patch("generate_phase1l_corpus.shutil.disk_usage",
                          return_value=SimpleNamespace(free=RESERVE + BUDGET + 1))
        self.disk.start()
        self.addCleanup(self.disk.stop)

    def tree(self):
        return OwnedTree(self.root, create=True)

    def test_plan_is_bounded_and_covers_requested_matrix(self):
        result = plan()
        self.assertEqual(result["stress_1000_paths"]["objects"], 1000)
        self.assertEqual(result["stress_1000_paths"]["unique_payloads"], 25)
        self.assertEqual(result["stress_250_unique"]["unique_payloads"], 250)
        self.assertEqual(result["mixed_prod_1000"]["objects"], 1000)
        self.assertEqual(result["estimated_encoded_bytes"],
                         sum(result["estimated_components_bytes"].values()))
        self.assertLessEqual(result["estimated_encoded_bytes"], BUDGET)
        self.assertEqual([row["jpeg"][0] for row in result["huge_sweep"]], list(AXES))
        self.assertEqual(result["huge_sweep"][-1]["jpeg"], [50000, 512])
        for width, height in zip(AXES, JPEG_HEIGHTS):
            channels = 1 if width * height * 4 > 64 * 1024**2 else 4
            self.assertLessEqual(width * height * channels, 64 * 1024**2)

    def test_same_seed_produces_same_payloads_layout_and_real_hardlinks(self):
        tree = self.tree()
        first = generate_stress(tree, "small_a", 7, 2, [(120, 90)], 42)
        second = generate_stress(tree, "small_b", 7, 2, [(120, 90)], 42)
        self.assertEqual([s["sha256"] for s in first["sources"]],
                         [s["sha256"] for s in second["sources"]])
        self.assertNotEqual(first["sources"][0]["sha256"], first["sources"][1]["sha256"])
        self.assertEqual(first["filesystem"]["repeated_hardlink_paths"], 5)
        self.assertEqual(len({o["path"] for o in first["objects"]}), 7)
        for a, b in zip(first["objects"], second["objects"]):
            path = tree.root / a["path"]
            with Image.open(path) as image:
                self.assertEqual(image.size, (120, 90))
                self.assertEqual(image.format, "JPEG")
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), a["source_sha256"])
            self.assertEqual([a[k] for k in ("x", "y", "width", "height")],
                             [b[k] for k in ("x", "y", "width", "height")])
        self.assertEqual(os.stat(tree.root / first["objects"][0]["path"]).st_ino,
                         os.stat(tree.root / first["objects"][2]["path"]).st_ino)
        OwnedTree(self.root).cleanup()
        self.assertFalse(self.root.exists())

    def test_axis_manifests_reuse_sources_and_multi_png_bands_do_not_overlap(self):
        huge = {"objects": []}
        for index, axis in enumerate(AXES):
            for suffix, height in (("jpg", JPEG_HEIGHTS[index]),
                                   ("png", PNG_HEIGHTS[index])):
                huge["objects"].append({"id": len(huge["objects"]),
                    "path": f"huge_sweep/{axis}x{height}.{suffix}",
                    "source_sha256": "fixture", "source_width": axis,
                    "source_height": height, "x": 100, "y": 200,
                    "width": 1500, "height": 1500 * height / axis})
        original = json.dumps(huge, sort_keys=True)
        values = extra_huge_manifests(huge)
        self.assertEqual(json.dumps(huge, sort_keys=True), original)
        self.assertEqual(set(values), set(axis_names()) |
                         {"several_huge_png", "overlapped_huge_png"})
        original_paths = {obj["path"] for obj in huge["objects"]}
        for name in axis_names():
            obj, = values[name]["objects"]
            self.assertIn(obj["path"], original_paths)
            self.assertEqual((obj["id"], obj["x"], obj["y"]), (0, 0, 0))
        bands = values["several_huge_png"]["objects"]
        self.assertEqual([obj["source_width"] for obj in bands], [24000, 32000, 50000])
        for previous, current in zip(bands, bands[1:]):
            self.assertLess(previous["y"] + previous["height"], current["y"])
            self.assertTrue(current["path"].endswith(".png"))
        overlap = values["overlapped_huge_png"]["objects"]
        self.assertEqual([obj["y"] for obj in overlap], [0, 16, 32])
        for source, obj in zip(bands, overlap):
            self.assertEqual(obj["path"], source["path"])
            self.assertEqual(obj["height"], source["height"])
            self.assertEqual((obj["x"], obj["width"]), (0, 1500))
        tree = self.tree()
        self.assertEqual(len(write_extra_huge_manifests(tree, huge)), 18)
        self.assertEqual(len(list(tree.root.glob("axis-*.json"))), 16)
        with self.assertRaises(ValueError):
            write_extra_huge_manifests(tree, huge)
        OwnedTree(self.root).validate()
        with self.assertRaises(ValueError):
            extra_huge_manifests({"objects": huge["objects"][:-1]})

    def test_png_is_static_noninterlaced_and_matches_procedural_source_samples(self):
        tree = self.tree()
        path = tree.register("detail.png")
        png(path, 319, 79, 71)
        contents = path.read_bytes()
        self.assertEqual(contents[:8], b"\x89PNG\r\n\x1a\n")
        self.assertEqual(struct.unpack(">IIBBBBB", contents[16:29]), (319, 79, 8, 2, 0, 0, 0))
        kinds, cursor = [], 8
        while cursor < len(contents):
            size = struct.unpack(">I", contents[cursor:cursor + 4])[0]
            kinds.append(contents[cursor + 4:cursor + 8])
            cursor += 12 + size
        self.assertNotIn(b"acTL", kinds)
        with Image.open(path) as image:
            image.load()
            for x, y in ((0, 0), (33, 32), (159, 12), (318, 78)):
                row = procedural_row(319, y, 71)
                self.assertEqual(image.getpixel((x, y)), tuple(row[x * 3:x * 3 + 3]))
        other = tree.register("same.png")
        png(other, 319, 79, 71)
        self.assertEqual(contents, other.read_bytes())
        wide_row = procedural_row(8200, 64, 71)
        self.assertNotEqual(wide_row[:3], wide_row[8192 * 3:8192 * 3 + 3])

    def test_mixed_huge_visible_geometry_reuses_existing_source_records(self):
        source = {"path": "huge_sweep/50000x4096.png", "source_width": 50000,
                  "source_height": 4096, "source_sha256": "giant"}
        mixed = {"objects": [{"path": f"ordinary-{i}.jpg", "source_width": 100,
                              "source_height": 200, "source_sha256": f"source-{i}"}
                             for i in range(4)]}
        original = json.dumps(mixed, sort_keys=True)
        result = mixed_huge_visible_manifest({"objects": [source]}, mixed)
        self.assertEqual(json.dumps(mixed, sort_keys=True), original)
        self.assertEqual(result["objects"][0]["height"], 122.88)
        self.assertEqual([(o["x"], o["y"]) for o in result["objects"][1:]],
                         [(730, 44), (756, 50), (728, 80), (768, 20)])
        self.assertTrue(all(o["width"] == 35 and o["height"] == 70
                            for o in result["objects"][1:]))

    def test_crossover_generation_is_row_streamed_and_registered(self):
        tree = self.tree()
        with patch("generate_phase1l_corpus.CROSSOVER_AXES", (64, 96, 128)):
            names = generate_crossover(tree)
        self.assertEqual(names, ("crossover-64-png", "crossover-96-png", "crossover-128-png"))
        for axis, name in zip((64, 96, 128), names):
            value = json.loads((tree.root / (name + ".json")).read_text())
            obj, = value["objects"]
            path = tree.root / obj["path"]
            self.assertEqual(obj["source_sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
            with Image.open(path) as image:
                self.assertEqual(image.size, (axis, axis))
                self.assertEqual(image.getpixel((33, 32)),
                    tuple(procedural_row(axis, 32, value["seed"])[99:102]))
        OwnedTree(self.root).validate()

    def test_preserve_existing_data_and_refuse_wrong_root_or_symlink_ancestry(self):
        user = self.parent / "test_file"
        user.write_bytes(b"owner board")
        self.root.mkdir()
        (self.root / "owner.txt").write_text("keep")
        with self.assertRaises(ValueError):
            self.tree()
        self.assertEqual((self.root / "owner.txt").read_text(), "keep")
        with self.assertRaises(ValueError):
            guarded_root(self.parent / "other_generated")
        alias = Path(self.temp.name) / "alias"
        alias.symlink_to(self.parent, target_is_directory=True)
        with self.assertRaises(ValueError):
            guarded_root(alias / "phase1l_generated")
        self.assertEqual(user.read_bytes(), b"owner board")

    def test_cleanup_refuses_unowned_files_before_removing_any_owned_file(self):
        tree = self.tree()
        tree.register("keep-owned").write_text("generated")
        (tree.root / "owner-added").write_text("user data")
        with self.assertRaises(ValueError):
            OwnedTree(self.root).cleanup()
        self.assertEqual((tree.root / "keep-owned").read_text(), "generated")
        self.assertEqual((tree.root / "owner-added").read_text(), "user data")

    def test_cleanup_requires_sentinel_and_rejects_escape_entries_and_symlinks(self):
        tree = self.tree()
        outside = self.parent / "owner"
        outside.write_text("preserve")
        (tree.root / SENTINEL).write_text("unrecognized\n")
        with self.assertRaises(ValueError):
            OwnedTree(self.root)
        (tree.root / SENTINEL).write_text(OWNER + "\n")
        tree.state["files"].append("../owner")
        tree.save()
        with self.assertRaises(ValueError):
            OwnedTree(self.root)
        tree.state["files"].remove("../owner")
        tree.save()
        path = tree.register("alias")
        path.symlink_to(outside)
        with self.assertRaises(ValueError):
            OwnedTree(self.root).cleanup()
        self.assertEqual(outside.read_text(), "preserve")
        for value in ("", ".", "..", "../owner", "/absolute"):
            with self.assertRaises(ValueError):
                relative_path(value)

    def test_partial_generation_is_journaled_and_removable(self):
        tree = self.tree()
        tree.register("images", directory=True)
        tree.register("images/not-written.jpg")
        tree.register("images/partial.jpg").write_bytes(b"partial")
        on_disk = json.loads((self.root / OWNERSHIP).read_text())
        self.assertIn("images/not-written.jpg", on_disk["files"])
        OwnedTree(self.root).cleanup()
        self.assertFalse(self.root.exists())

    def test_low_disk_and_budget_stop_writes(self):
        with patch("generate_phase1l_corpus.shutil.disk_usage",
                   return_value=SimpleNamespace(free=RESERVE + BUDGET - 1)):
            with self.assertRaises(ValueError):
                self.tree()
        self.assertFalse(self.root.exists())
        tree = self.tree()
        with patch("generate_phase1l_corpus.shutil.disk_usage",
                   return_value=SimpleNamespace(free=RESERVE - 1)):
            with self.assertRaises(ValueError):
                tree.admit()
        with patch("generate_phase1l_corpus.tree_stats", return_value={"physical_du_bytes": BUDGET}):
            with self.assertRaises(ValueError):
                tree.admit(1)

    def test_hardlink_failure_never_falls_back_to_copy(self):
        tree = self.tree()
        with patch("generate_phase1l_corpus.os.link", side_effect=OSError("unsupported")):
            with self.assertRaises(OSError):
                generate_stress(tree, "small", 2, 1, [(80, 60)], 4)
        self.assertFalse((self.root / "small/image-0001.jpg").exists())
        OwnedTree(self.root).cleanup()

    def test_jpeg_rejects_unbounded_working_image_before_creating_output(self):
        path = self.parent / "never-created.jpg"
        with self.assertRaises(ValueError):
            jpeg(path, 50000, 4096, 0)
        self.assertFalse(path.exists())


if __name__ == "__main__":
    unittest.main()
