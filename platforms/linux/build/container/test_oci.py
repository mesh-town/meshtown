"""Tests of oci.py: the layer, the image config, blob digests, the index and the layout on disk.

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/container -t platforms/linux/build/container
"""

import gzip
import io
import json
import tarfile
import unittest
from pathlib import Path

import archive
import oci
from image_fixtures import (AARCH64, ALL, ARMV7, DIGEST, SUFFIX, VERSION, X86_64, blob_path, elf_bytes, read_json,
                            sample_images, sha256_hex, tmpdir, tree)

EXPECTED_ARCH = {AARCH64: ("arm64", None), ARMV7: ("arm", "v7"), X86_64: ("amd64", None)}
LAYER_ENTRIES = [("usr", 0o755), ("usr/bin", 0o755), ("usr/bin/meshtown", 0o755), ("data", 0o700)]
MANIFEST_TYPE = "application/vnd.oci.image.manifest.v1+json"
INDEX_TYPE = "application/vnd.oci.image.index.v1+json"
CONFIG_TYPE = "application/vnd.oci.image.config.v1+json"
LAYER_TYPE = "application/vnd.oci.image.layer.v1.tar+gzip"
SOURCE = "https://github.com/mesh-town/meshtown"


class Layer(unittest.TestCase):
    def test_layer_entries_modes_and_owners(self):
        # given
        binary = elf_bytes("aarch64")
        # when
        entries = oci.layer_entries(binary)
        data = oci.tar_bytes(entries)
        # then
        self.assertEqual([(e.name.rstrip("/"), e.mode) for e in entries], LAYER_ENTRIES)
        self.assertEqual(entries[2].data, binary)
        members = read_members(data)
        self.assertEqual([(m.name, m.type, m.mode & 0o7777) for m in members],
                         [("usr", tarfile.DIRTYPE, 0o755), ("usr/bin", tarfile.DIRTYPE, 0o755),
                          ("usr/bin/meshtown", tarfile.REGTYPE, 0o755), ("data", tarfile.DIRTYPE, 0o700)])
        for m in members:
            self.assertEqual((m.uid, m.gid, m.uname, m.gname, m.mtime), (0, 0, "", "", 0), m.name)

    def test_tar_is_plain_ustar_and_repeatable(self):
        # given
        entries = oci.layer_entries(elf_bytes("x86_64"))
        # when
        first, second = oci.tar_bytes(entries), oci.tar_bytes(entries)
        # then
        self.assertEqual(first, second)
        self.assertEqual(first[257:265], b"ustar\x0000", "the first header must be USTAR, not GNU or PAX")

    def test_tar_bytes_writes_a_symlink_entry(self):
        # given
        entries = [oci.Entry("a.tar", 0o444, b"data"), oci.Entry("dir/layer.tar", 0o444, None, "../a.tar")]
        # when
        members = read_members(oci.tar_bytes(entries))
        # then
        self.assertTrue(members[1].issym())
        self.assertEqual(members[1].linkname, "../a.tar")

    def test_gzip_is_repeatable_and_carries_no_name_or_time(self):
        # given
        data = bytes(range(256)) * 40
        # when
        first, second = oci.gzip_bytes(data), oci.gzip_bytes(data)
        # then
        self.assertEqual(first, second)
        self.assertEqual(gzip.decompress(first), data)
        self.assertEqual(first[4:8], b"\0\0\0\0", "gzip mtime must be zero")
        self.assertEqual(first[3] & 0x08, 0, "gzip must carry no file name")


class Config(unittest.TestCase):
    def test_config_fields_per_platform(self):
        for image in sample_images():
            triple = image.platform.triple
            with self.subTest(triple=triple):
                # given
                arch, variant = EXPECTED_ARCH[triple]
                # when
                cfg = json.loads(image.config.data)
                # then
                self.assertEqual(image.config.media_type, CONFIG_TYPE)
                self.assertEqual((cfg["architecture"], cfg["os"], cfg.get("variant")), (arch, "linux", variant))
                self.assertEqual(sorted(set(cfg) - {"variant"}), ["architecture", "config", "os", "rootfs"],
                                 "no created, author or history: builds must not depend on who made them")
                self.assert_runtime_config(cfg["config"])

    def assert_runtime_config(self, config):
        self.assertEqual(config["Entrypoint"], ["/usr/bin/meshtown"])
        self.assertEqual(config["Cmd"], ["run", "--socks-access", "localnet"])
        self.assertEqual(config["Env"], ["PATH=/usr/bin", "MESHTOWN_CONFIG=/data/config.json", "NO_COLOR=1"])
        self.assertEqual(config["Volumes"], {"/data": {}})
        self.assertEqual(config["ExposedPorts"], {"7777/tcp": {}, "1080/tcp": {}})
        self.assertEqual(config["StopSignal"], "SIGTERM")
        self.assertFalse(config.get("User"), "the volume of the router belongs to root")
        self.assertFalse(config.get("WorkingDir"))
        labels = config["Labels"]
        self.assertEqual({k: v for k, v in labels.items() if not k.endswith(".description")}, {
            "org.opencontainers.image.title": "meshtown",
            "org.opencontainers.image.version": VERSION,
            "org.opencontainers.image.licenses": "AGPL-3.0-only",
            "org.opencontainers.image.source": SOURCE})
        self.assertTrue(labels["org.opencontainers.image.description"].strip())

    def test_diff_id_matches_uncompressed_layer(self):
        for image in sample_images():
            with self.subTest(triple=image.platform.triple):
                # given
                cfg = json.loads(image.config.data)
                # when
                unpacked = gzip.decompress(image.layer.data)
                # then
                self.assertEqual(unpacked, image.layer_tar)
                self.assertEqual(cfg["rootfs"], {"type": "layers", "diff_ids": ["sha256:" + sha256_hex(image.layer_tar)]})
                self.assertEqual(image.layer.media_type, LAYER_TYPE)

    def test_manifest_points_to_config_and_layer(self):
        for image in sample_images():
            with self.subTest(triple=image.platform.triple):
                # given
                manifest = json.loads(image.manifest.data)
                # then
                self.assertEqual((manifest["schemaVersion"], manifest["mediaType"]), (2, MANIFEST_TYPE))
                self.assertEqual(manifest["config"], descriptor(image.config))
                self.assertEqual(manifest["layers"], [descriptor(image.layer)])


class Blobs(unittest.TestCase):
    def test_blob_digest_and_size(self):
        # when
        blob = oci.blob("text/plain", b"abc")
        # then
        self.assertEqual((blob.media_type, blob.size, blob.data), ("text/plain", 3, b"abc"))
        self.assertEqual(blob.digest, "sha256:" + sha256_hex(b"abc"))

    def test_json_bytes_is_sorted_and_compact(self):
        # when
        data = oci.json_bytes({"b": [1, {"d": 1, "c": "é"}], "a": 0})
        # then
        self.assertEqual(data.decode("utf-8"), '{"a":0,"b":[1,{"c":"é","d":1}]}')

    def test_blob_digests_and_sizes(self):
        # given
        root = written_layout(self)
        # when
        files = tree(root / "blobs" / "sha256")
        # then
        self.assertTrue(files)
        for name, content in files.items():
            self.assertEqual(name, sha256_hex(content), "a blob is named by the digest of its content")
        for desc in reachable_descriptors(root):
            self.assertRegex(desc["digest"], DIGEST)
            content = blob_path(root, desc["digest"]).read_bytes()
            self.assertEqual(desc["size"], len(content), desc["digest"])


class Layout(unittest.TestCase):
    def test_layout_index_refs(self):
        # given
        root = written_layout(self)
        # when
        index = read_json(root / "index.json")
        refs = {d["annotations"]["org.opencontainers.image.ref.name"]: d for d in index["manifests"]}
        # then
        self.assertEqual(index["schemaVersion"], 2)
        self.assertEqual(sorted(refs), sorted([VERSION, *[f"{VERSION}-{SUFFIX[t]}" for t in ALL]]))
        self.assertEqual(refs[VERSION]["mediaType"], INDEX_TYPE)
        for image in sample_images():
            ref = refs[f"{VERSION}-{SUFFIX[image.platform.triple]}"]
            self.assertEqual((ref["mediaType"], ref["digest"], ref["size"]),
                             (MANIFEST_TYPE, image.manifest.digest, image.manifest.size))
            self.assertEqual(ref["platform"], expected_platform(image.platform.triple))

    def test_multi_platform_index_lists_every_architecture(self):
        # given
        root = written_layout(self)
        index = read_json(root / "index.json")
        top = next(d for d in index["manifests"] if d["mediaType"] == INDEX_TYPE)
        # when
        listed = json.loads(blob_path(root, top["digest"]).read_bytes())
        # then
        self.assertEqual((listed["schemaVersion"], listed["mediaType"]), (2, INDEX_TYPE))
        self.assertEqual([m["platform"] for m in listed["manifests"]], [expected_platform(t) for t in EXPECTED_ARCH])
        self.assertEqual([m["digest"] for m in listed["manifests"]], [i.manifest.digest for i in sample_images()])
        self.assertEqual(listed["annotations"]["org.opencontainers.image.source"], SOURCE)
        self.assertTrue(listed["annotations"]["org.opencontainers.image.description"].strip())

    def test_layout_has_oci_layout_marker(self):
        # when
        root = written_layout(self)
        # then
        self.assertEqual(read_json(root / "oci-layout"), {"imageLayoutVersion": "1.0.0"})

    def test_two_builds_are_byte_identical(self):
        # given
        first, second = sample_images(), sample_images()
        # when
        trees = [tree(written_layout(self, images)) for images in (first, second)]
        # then
        self.assertEqual(trees[0], trees[1])
        self.assertEqual([i.layer_tar for i in first], [i.layer_tar for i in second])
        for name, write in archive.WRITERS.items():
            self.assertEqual([write(i, VERSION) for i in first], [write(i, VERSION) for i in second], name)


# ---------- setup and helpers ----------

def written_layout(test, images=None):
    root = tmpdir(test) / "oci"
    oci.write_layout(root, VERSION, images or sample_images())
    return root


def read_members(data):
    with tarfile.open(fileobj=io.BytesIO(data)) as tar:
        return tar.getmembers()


def descriptor(blob):
    return {"mediaType": blob.media_type, "digest": blob.digest, "size": blob.size}


def expected_platform(triple):
    arch, variant = EXPECTED_ARCH[triple]
    return {"architecture": arch, "os": "linux", **({"variant": variant} if variant else {})}


def reachable_descriptors(root):
    """Every descriptor of the layout: the top index, the multi-platform index, manifests, configs and layers."""
    found, pending = [], list(read_json(Path(root) / "index.json")["manifests"])
    while pending:
        desc = pending.pop()
        found.append(desc)
        if desc["mediaType"] in (INDEX_TYPE, MANIFEST_TYPE):
            body = json.loads(blob_path(root, desc["digest"]).read_bytes())
            pending += body.get("manifests", []) + body.get("layers", []) + ([body["config"]] if "config" in body else [])
    return found


if __name__ == "__main__":
    unittest.main()
