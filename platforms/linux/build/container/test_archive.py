"""Tests of archive.py and of the tars and sums file build_image.py writes (no external tools).

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/container -t platforms/linux/build/container
"""

import json
import re
import unittest

import archive
import image_spec
from image_fixtures import (ALL, IMAGE, SUFFIX, VERSION, blob_path, built_ok, sample_images, sha256_hex, tar_entries,
                            tar_name)

HEX = re.compile(r"^[0-9a-f]{64}$")
SUMS_LINE = re.compile(r"^([0-9a-f]{64})  (\S+)$")


class DockerFormat(unittest.TestCase):
    def test_docker_tar_layout(self):
        for image in sample_images():
            suffix = SUFFIX[image.platform.triple]
            with self.subTest(suffix=suffix):
                # given
                repo_tag = f"{IMAGE}:{VERSION}-{suffix}"
                # when
                members = tar_entries(archive.docker_tar(image, repo_tag))
                # then
                names = [name for name, _, _ in members]
                legacy = names[2].split("/")[0]
                diff_hex, cfg_hex = sha256_hex(image.layer_tar), sha256_hex(image.config.data)
                self.assertRegex(legacy, HEX)
                self.assertEqual(names, [f"{diff_hex}.tar", f"{cfg_hex}.json", f"{legacy}/layer.tar",
                                         f"{legacy}/VERSION", f"{legacy}/json", "manifest.json", "repositories"])
                self.assert_members_are_plain(members)
                self.assert_contents(dict((n, (i, d)) for n, i, d in members), image, repo_tag, legacy)

    def assert_members_are_plain(self, members):
        for name, info, _ in members:
            self.assertEqual((info.uid, info.gid, info.mtime), (0, 0, 0), name)
            self.assertFalse(info.isdir(), f"{name}: the archive has no directory entries")
            if not info.issym():
                self.assertEqual(info.mode & 0o7777, 0o444, name)

    def assert_contents(self, files, image, repo_tag, legacy):
        diff_hex, cfg_hex = sha256_hex(image.layer_tar), sha256_hex(image.config.data)
        self.assertEqual(files[f"{diff_hex}.tar"][1], image.layer_tar, "the layer is stored uncompressed")
        self.assertEqual(files[f"{cfg_hex}.json"][1], image.config.data)
        link = files[f"{legacy}/layer.tar"][0]
        self.assertTrue(link.issym())
        self.assertEqual(link.linkname, f"../{diff_hex}.tar")
        self.assertEqual(files[f"{legacy}/VERSION"][1], b"1.0")
        self.assertEqual(json.loads(files["manifest.json"][1]),
                         [{"Config": f"{cfg_hex}.json", "RepoTags": [repo_tag], "Layers": [f"{diff_hex}.tar"]}])
        repo, tag = repo_tag.rsplit(":", 1)
        self.assertEqual(json.loads(files["repositories"][1]), {repo: {tag: legacy}})
        self.assert_legacy_json(json.loads(files[f"{legacy}/json"][1]), json.loads(image.config.data), legacy)

    def assert_legacy_json(self, legacy_json, cfg, legacy):
        self.assertEqual(legacy_json["id"], legacy)
        self.assertEqual((legacy_json["architecture"], legacy_json["os"]), (cfg["architecture"], "linux"))
        self.assertEqual(legacy_json["config"], cfg["config"])
        self.assertNotIn("created", legacy_json)

    def test_manifest_members_exist_in_the_archive(self):
        for image in sample_images():
            with self.subTest(arch=image.platform.arch):
                # given
                members = tar_entries(archive.docker_tar(image, f"{IMAGE}:{VERSION}-x"))
                names = {name for name, _, _ in members}
                manifest = json.loads(dict((n, d) for n, _, d in members)["manifest.json"])
                # then
                for entry in manifest:
                    self.assertIn(entry["Config"], names)
                    self.assertTrue(set(entry["Layers"]) <= names)


class OciFormat(unittest.TestCase):
    def test_oci_tar_layout(self):
        for image in sample_images():
            suffix = SUFFIX[image.platform.triple]
            with self.subTest(suffix=suffix):
                # given
                ref = f"{VERSION}-{suffix}"
                # when
                members = tar_entries(archive.oci_tar(image, ref))
                # then
                files = {name: data for name, info, data in members if info.isfile()}
                blobs = {f"blobs/sha256/{b.digest.split(':', 1)[1]}": b for b in (image.layer, image.config, image.manifest)}
                self.assertEqual(sorted(files), sorted(["oci-layout", "index.json", *blobs]))
                self.assertEqual(json.loads(files["oci-layout"]), {"imageLayoutVersion": "1.0.0"})
                for name, blob in blobs.items():
                    self.assertEqual(files[name], blob.data, name)
                index = json.loads(files["index.json"])
                self.assertEqual(len(index["manifests"]), 1, "one manifest: this tar holds a single architecture")
                self.assert_manifest_entry(index["manifests"][0], image, ref)

    def assert_manifest_entry(self, entry, image, ref):
        self.assertEqual(entry["digest"], image.manifest.digest)
        self.assertEqual(entry["size"], image.manifest.size)
        self.assertEqual(entry["annotations"]["org.opencontainers.image.ref.name"], ref)
        self.assertEqual(entry["platform"]["architecture"], json.loads(image.config.data)["architecture"])
        self.assertEqual(entry["platform"]["os"], "linux")


class Writers(unittest.TestCase):
    def test_writers_name_the_tag_after_version_and_architecture(self):
        self.assertEqual(sorted(archive.WRITERS), sorted(image_spec.TAR_FORMATS))
        for image in sample_images():
            suffix = SUFFIX[image.platform.triple]
            with self.subTest(suffix=suffix):
                # then
                self.assertEqual(archive.WRITERS["docker"](image, VERSION),
                                 archive.docker_tar(image, f"{IMAGE}:{VERSION}-{suffix}"))
                self.assertEqual(archive.WRITERS["oci"](image, VERSION), archive.oci_tar(image, f"{VERSION}-{suffix}"))


class Output(unittest.TestCase):
    def test_sums_file_covers_tars(self):
        for tar_format in image_spec.TAR_FORMATS:
            with self.subTest(tar_format=tar_format):
                # given
                result = built_ok(self, "--tar-format", tar_format)
                # when
                lines = (result.out / f"meshtown-container-{VERSION}.sha256sums").read_text().splitlines()
                # then
                parsed = [SUMS_LINE.match(line) for line in lines]
                self.assertTrue(all(parsed), f"every line must read '<sha256>  <name>': {lines}")
                self.assertEqual(sorted(m.group(2) for m in parsed), sorted(tar_name(VERSION, t) for t in ALL))
                for match in parsed:
                    self.assertEqual(match.group(1), sha256_hex((result.out / match.group(2)).read_bytes()), match.group(2))

    def test_cli_builds_are_byte_identical(self):
        # given
        first, second = built_ok(self), built_ok(self)
        # then
        for triple in ALL:
            name = tar_name(VERSION, triple)
            self.assertEqual((first.out / name).read_bytes(), (second.out / name).read_bytes(), name)
        sums = f"meshtown-container-{VERSION}.sha256sums"
        self.assertEqual((first.out / sums).read_bytes(), (second.out / sums).read_bytes())

    def test_oci_layout_of_the_output_is_complete(self):
        # given
        result = built_ok(self)
        # when
        root = result.out / "oci"
        # then
        for image in sample_images():
            self.assertTrue(blob_path(root, image.manifest.digest).is_file(), image.platform.triple)
        self.assertTrue((root / "oci-layout").is_file())
        self.assertTrue((root / "index.json").is_file())


if __name__ == "__main__":
    unittest.main()
