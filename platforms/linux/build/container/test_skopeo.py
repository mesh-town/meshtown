"""Tests that skopeo, an independent reader of container images, accepts what build_image.py writes.

Without skopeo the tests are skipped; MESHTOWN_IMAGE_REQUIRE_SKOPEO=1 turns a missing skopeo into a failure.
Run from the tree root: python3 -m unittest discover -s platforms/linux/build/container -t platforms/linux/build/container
"""

import json
import re
import unittest

import image_spec
from image_fixtures import (ALL, IMAGE, SUFFIX, VERSION, blob_path, built_ok, read_json, run_skopeo, skopeo_json,
                            skopeo_or_skip, tar_entries, tar_name, tmpdir)

ARCH = {"arm64": "arm64", "arm": "arm", "amd64": "amd64"}
HEX = re.compile(r"[0-9a-f]{64}")


class Layout(unittest.TestCase):
    def test_skopeo_reads_layout(self):
        # given
        skopeo = skopeo_or_skip(self)
        result = built_ok(self)
        for suffix in ARCH.values():
            with self.subTest(suffix=suffix):
                # when
                info = skopeo_json(skopeo, "inspect", f"oci:{result.out}/oci:{VERSION}-{suffix}")
                # then
                self.assertEqual((info["Architecture"], info["Os"]), (suffix, "linux"))
                self.assertIn("NO_COLOR=1", info["Env"])
                self.assertIn("PATH=/usr/bin", info["Env"])

    def test_skopeo_reads_the_variant_of_armv7(self):
        # given
        skopeo = skopeo_or_skip(self)
        result = built_ok(self)
        # when
        config = skopeo_json(skopeo, "inspect", "--config", f"oci:{result.out}/oci:{VERSION}-arm")
        # then
        self.assertEqual((config["architecture"], config["variant"]), ("arm", "v7"))

    def test_skopeo_lists_every_platform_of_the_index(self):
        # given
        skopeo = skopeo_or_skip(self)
        result = built_ok(self)
        # when
        raw = skopeo_json(skopeo, "inspect", "--raw", f"oci:{result.out}/oci:{VERSION}")
        # then
        platforms = [(m["platform"]["architecture"], m["platform"].get("variant")) for m in raw["manifests"]]
        self.assertEqual(platforms, [("arm64", None), ("arm", "v7"), ("amd64", None)])

    def test_skopeo_copies_index(self):
        # given
        skopeo = skopeo_or_skip(self)
        result = built_ok(self)
        target = tmpdir(self) / "copy"
        # when
        run_skopeo(skopeo, "copy", "--all", "--preserve-digests", f"oci:{result.out}/oci:{VERSION}", f"dir:{target}")
        # then
        index = read_json(result.out / "oci" / "index.json")
        top = next(d for d in index["manifests"] if d["annotations"]["org.opencontainers.image.ref.name"] == VERSION)
        self.assertEqual((target / "manifest.json").read_bytes(), blob_path(result.out / "oci", top["digest"]).read_bytes(),
                         "skopeo must copy the index unchanged")


class Archives(unittest.TestCase):
    def test_skopeo_reads_both_tar_formats(self):
        reader = {"docker": "docker-archive", "oci": "oci-archive"}
        skopeo = skopeo_or_skip(self)
        for tar_format in image_spec.TAR_FORMATS:
            with self.subTest(tar_format=tar_format):
                # given
                result = built_ok(self, "--tar-format", tar_format)
                for triple in ALL:
                    # when
                    info = skopeo_json(skopeo, "inspect", f"{reader[tar_format]}:{result.out / tar_name(VERSION, triple)}")
                    # then
                    self.assertEqual((info["Architecture"], info["Os"]), (ARCH[SUFFIX[triple]], "linux"), triple)
                    self.assertIn("NO_COLOR=1", info["Env"])

    def test_docker_tar_matches_skopeo_shape(self):
        # given
        skopeo = skopeo_or_skip(self)
        result = built_ok(self, "--tar-format", "docker")
        reference = tmpdir(self) / "reference.tar"
        tag = f"{IMAGE}:{VERSION}-arm64"
        # when
        run_skopeo(skopeo, "copy", f"oci:{result.out}/oci:{VERSION}-arm64", f"docker-archive:{reference}:{tag}")
        # then
        ours = tar_entries((result.out / tar_name(VERSION, ALL[0])).read_bytes())
        theirs = tar_entries(reference.read_bytes())
        self.assertEqual(shape(ours), shape(theirs), "members, their kinds and modes must follow skopeo's docker-archive")
        self.assertEqual(manifest_keys(ours), manifest_keys(theirs))
        self.assertEqual(json.loads(member(ours, "manifest.json"))[0]["RepoTags"], [tag])
        self.assertEqual(repositories_shape(ours), repositories_shape(theirs))


# ---------- helpers ----------

def shape(members):
    """[(name with every 64-digit hex replaced by <hex>, type, permission bits)] in archive order."""
    return [(HEX.sub("<hex>", name), info.type, info.mode & 0o7777) for name, info, _ in members]


def member(members, name):
    return next(data for n, _, data in members if n == name)


def manifest_keys(members):
    return sorted(json.loads(member(members, "manifest.json"))[0])


def repositories_shape(members):
    """{repository: {tag: <hex>}} with the image id masked."""
    repos = json.loads(member(members, "repositories"))
    return {repo: {tag: HEX.sub("<hex>", ident) for tag, ident in tags.items()} for repo, tags in repos.items()}


if __name__ == "__main__":
    unittest.main()
