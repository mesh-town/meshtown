"""Tests of build_image.py: release archives to an OCI layout, per-architecture tars and a sums file.

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/container -t platforms/linux/build/container
"""

import unittest

import image_spec
from image_fixtures import (ALL, AARCH64, ARMV7, DIGEST, MIPS, MIPSEL, PLATFORM_NAME, VERSION, X86_64,
                            blob_path, fetched, platform_lines, release_archive, run_builder, tar_entries,
                            tar_name, tmpdir)

OTHER_VERSION = "0.3.194"
WRONG_MACHINE = {AARCH64: "x86_64", ARMV7: "aarch64", X86_64: "aarch64"}


class Inputs(unittest.TestCase):
    def test_three_archives_give_three_platforms(self):
        # given
        run = setup(self, ALL)
        # when
        done = run.when_built()
        # then
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        lines = platform_lines(done.stdout)
        self.assertEqual([line[0] for line in lines], [PLATFORM_NAME[t] for t in ALL])
        self.assertEqual([line[2] for line in lines], [tar_name(VERSION, t) for t in ALL])
        for _, digest, _ in lines:
            self.assertRegex(digest, DIGEST)
            self.assertTrue(blob_path(run.out / "oci", digest).is_file(), f"{digest} is not a blob of the layout")
        self.assertEqual(run.listing(), sorted(["oci", f"meshtown-container-{VERSION}.sha256sums",
                                                *[tar_name(VERSION, t) for t in ALL]]))

    def test_missing_armv7_is_skipped_with_warning(self):
        # given
        run = setup(self, [AARCH64, X86_64])
        # when
        done = run.when_built()
        # then
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual([line[0] for line in platform_lines(done.stdout)], [PLATFORM_NAME[AARCH64], PLATFORM_NAME[X86_64]])
        self.assertIn("::warning::", done.stdout)
        self.assertIn(f"meshtown-cli-{VERSION}-{ARMV7}.tar.gz", done.stdout)
        self.assertNotIn("::error::", done.stdout)
        self.assertNotIn(tar_name(VERSION, ARMV7), run.listing())

    def test_missing_required_arch_fails(self):
        for missing in (AARCH64, X86_64):
            with self.subTest(missing=missing):
                # given
                run = setup(self, [t for t in ALL if t != missing])
                # when
                done = run.when_built()
                # then
                self.assert_refused(done)

    def test_no_archives_fails(self):
        # given
        run = setup(self, [])
        # when
        done = run.when_built()
        # then
        self.assert_refused(done)

    def test_mips_archives_are_ignored(self):
        # given
        run = setup(self, [*ALL, MIPS, MIPSEL])
        # when
        done = run.when_built()
        # then
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
        self.assertEqual(len(platform_lines(done.stdout)), 3)
        self.assertEqual(len(run.listing()), 5, "only the oci layout, three tars and the sums file are written")

    def test_archive_of_other_version_is_not_used(self):
        for others, code in (([ARMV7], 0), ([AARCH64], 1)):
            with self.subTest(other_version=others):
                # given
                run = setup(self, [t for t in ALL if t not in others])
                for triple in others:
                    release_archive(run.fetched, OTHER_VERSION, triple)
                # when
                done = run.when_built()
                # then
                self.assertEqual(done.returncode, code, done.stdout + done.stderr)
                if code == 0:
                    self.assertNotIn(OTHER_VERSION, "".join(run.listing()))

    def assert_refused(self, done):
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("::error::", done.stdout)
        self.assertNotIn("Traceback", done.stderr)


class Binary(unittest.TestCase):
    def test_dynamic_binary_is_refused(self):
        for triple in ALL:
            with self.subTest(triple=triple):
                # given
                run = setup(self, ALL, {triple: {"static": False}})
                # when
                done = run.when_built()
                # then
                self.assert_refused_for(done, triple)

    def test_wrong_machine_is_refused(self):
        for triple in ALL:
            with self.subTest(triple=triple):
                # given: the archive of this target holds the binary of another machine
                run = setup(self, ALL, {triple: {"machine": WRONG_MACHINE[triple]}})
                # when
                done = run.when_built()
                # then
                self.assert_refused_for(done, triple)

    def test_non_elf_is_refused(self):
        for triple in ALL:
            with self.subTest(triple=triple):
                # given
                run = setup(self, ALL, {triple: {"raw": b"#!/bin/sh\necho not an elf file\n"}})
                # when
                done = run.when_built()
                # then
                self.assert_refused_for(done, triple)

    def test_archive_without_binary_fails(self):
        for triple in ALL:
            with self.subTest(triple=triple):
                # given
                run = setup(self, ALL, {triple: {"member": "README"}})
                # when
                done = run.when_built()
                # then
                self.assert_refused_for(done, triple)

    def test_corrupt_archive_fails(self):
        for triple in ALL:
            with self.subTest(triple=triple):
                # given
                run = setup(self, ALL)
                (run.fetched / f"meshtown-cli-{VERSION}-{triple}.tar.gz").write_bytes(b"this is not a gzip stream")
                # when
                done = run.when_built()
                # then
                self.assert_refused_for(done, triple)

    def assert_refused_for(self, done, triple):
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("::error::", done.stdout)
        self.assertIn(triple, done.stdout, "the message must name the target")
        self.assertNotIn("Traceback", done.stderr)


class Arguments(unittest.TestCase):
    def test_unknown_tar_format_is_usage_error(self):
        # given
        run = setup(self, ALL)
        # when
        done = run.when_built("--tar-format", "zip")
        # then
        self.assertEqual(done.returncode, 2, done.stdout + done.stderr)
        self.assertFalse(run.out.exists(), "a usage error must write nothing")

    def test_bad_version_is_usage_error(self):
        for version in ("v1.2", "1.2", "v0.3.195", "1.2.x"):
            with self.subTest(version=version):
                # given
                run = setup(self, ALL)
                # when
                done = run_builder("--fetched", run.fetched, "--version", version, "--out", run.out)
                # then
                self.assertEqual(done.returncode, 2, done.stdout + done.stderr)
                self.assertFalse(run.out.exists(), "a usage error must write nothing")

    def test_non_empty_out_is_refused(self):
        # given
        run = setup(self, ALL)
        run.out.mkdir()
        (run.out / "keep.txt").write_text("an earlier build\n")
        # when
        done = run.when_built()
        # then
        self.assertEqual(done.returncode, 1, done.stdout + done.stderr)
        self.assertIn("::error::", done.stdout)
        self.assertEqual(run.listing(), ["keep.txt"], "an earlier build must stay untouched")

    def test_empty_out_is_accepted(self):
        # given
        run = setup(self, ALL)
        run.out.mkdir()
        # when
        done = run.when_built()
        # then
        self.assertEqual(done.returncode, 0, done.stdout + done.stderr)

    def test_default_tar_format_is_the_constant(self):
        # given
        run = setup(self, ALL)
        explicit = run.out.parent / "explicit"
        # when
        by_default = run.when_built()
        by_name = run.when_built("--tar-format", image_spec.DEFAULT_TAR_FORMAT, out=explicit)
        # then
        self.assertIn(image_spec.DEFAULT_TAR_FORMAT, image_spec.TAR_FORMATS)
        self.assertEqual((by_default.returncode, by_name.returncode), (0, 0), by_default.stderr + by_name.stderr)
        for triple in ALL:
            name = tar_name(VERSION, triple)
            self.assertEqual((run.out / name).read_bytes(), (explicit / name).read_bytes(), name)

    def test_tar_format_flag_selects_the_layout(self):
        marker = {"docker": "manifest.json", "oci": "oci-layout"}
        other = {"docker": "oci-layout", "oci": "manifest.json"}
        for tar_format in image_spec.TAR_FORMATS:
            with self.subTest(tar_format=tar_format):
                # given
                run = setup(self, ALL)
                # when
                done = run.when_built("--tar-format", tar_format)
                # then
                self.assertEqual(done.returncode, 0, done.stdout + done.stderr)
                names = [name for name, _, _ in tar_entries((run.out / tar_name(VERSION, AARCH64)).read_bytes())]
                self.assertIn(marker[tar_format], names)
                self.assertNotIn(other[tar_format], names)


# ---------- setup: a fetched directory and a runner ----------

class Run:
    def __init__(self, tmp, directory):
        self.fetched, self.out = directory, tmp / "out"

    def when_built(self, *flags, out=None):
        return run_builder("--fetched", self.fetched, "--version", VERSION, "--out", out or self.out, *flags)

    def listing(self):
        return sorted(p.name for p in self.out.iterdir())


def setup(test, triples, overrides=None):
    """Archives for `triples`; overrides maps a triple to release_archive arguments (a damaged binary)."""
    tmp = tmpdir(test)
    directory = fetched(tmp, VERSION, [t for t in triples if t not in (overrides or {})])
    for triple, options in (overrides or {}).items():
        release_archive(directory, VERSION, triple, **options)
    return Run(tmp, directory)


if __name__ == "__main__":
    unittest.main()
