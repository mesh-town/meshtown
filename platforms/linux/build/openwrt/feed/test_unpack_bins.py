"""Tests of unpack_bins.py: release archives to the `--bin` arguments of build_feed.py (no external tools).

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/openwrt/feed -t platforms/linux/build/openwrt/feed
"""

import contextlib
import io
import tarfile
import unittest
from collections import namedtuple
from unittest import mock

import targets
import unpack_bins
from feed_fixtures import AARCH64, ARMV7, MIPS, MIPSEL, X86_64, tmpdir

TAG = "v0.3.195"
VERSION = "0.3.195"
IN_ORDER = [AARCH64, X86_64, ARMV7, MIPS, MIPSEL]


class Strict(unittest.TestCase):
    def test_all_archives_give_one_bin_per_triple(self):
        for strict in ("true", "false"):
            with self.subTest(strict=strict):
                # given
                run = setup(self, IN_ORDER)
                # when
                done = run.when_unpacked(strict)
                # then
                self.assertEqual(done.code, 0, done.out)
                self.assertEqual(run.args(), expected_args(run, IN_ORDER))
                self.assertEqual(len(run.args()), 10)
                for triple in IN_ORDER:
                    self.assertTrue(run.binary(triple).is_file(), triple)
                    self.assertEqual(run.binary(triple).read_text(), f"binary of {triple}")

    def test_missing_mips_archive_fails_the_release(self):
        for missing in (MIPS, MIPSEL):
            with self.subTest(missing=missing):
                # given
                run = setup(self, [t for t in IN_ORDER if t != missing])
                # when
                done = run.when_unpacked("true")
                # then
                self.assertEqual(done.code, 1, done.out)
                self.assertIn(f"::error::release {TAG} has no meshtown-cli-{VERSION}-{missing}.tar.gz", done.out)
                self.assertFalse(run.args_file.exists(), "a failed release must leave no argument file")


class DryRun(unittest.TestCase):
    def test_missing_archive_is_skipped_with_a_warning(self):
        # given
        run = setup(self, [t for t in IN_ORDER if t != MIPS])
        # when
        done = run.when_unpacked("false")
        # then
        self.assertEqual(done.code, 0, done.out)
        self.assertIn(f"::warning::release {TAG} has no meshtown-cli-{VERSION}-{MIPS}.tar.gz; its labels are skipped", done.out)
        self.assertNotIn("::error::", done.out)
        self.assertEqual(run.args(), expected_args(run, [t for t in IN_ORDER if t != MIPS]))
        self.assertEqual(run.args().count("--bin"), 4)

    def test_no_archive_at_all_fails(self):
        for strict in ("true", "false"):
            with self.subTest(strict=strict):
                # given
                run = setup(self, [])
                # when
                done = run.when_unpacked(strict)
                # then
                self.assertEqual(done.code, 1, done.out)
                self.assertFalse(run.args_file.exists())
                if strict == "false":
                    self.assertIn("::error::no target archives found", done.out)


class TripleWithoutLabels(unittest.TestCase):
    def test_a_machine_known_to_the_builder_but_without_labels_does_not_fail_the_release(self):
        # given: another target is known to build_feed.py, its archive is absent, it has no label
        run = setup(self, IN_ORDER)
        extra = "riscv64gc-unknown-linux-musl"
        # when
        with mock.patch.dict(targets.TRIPLE_MACHINE, {extra: "riscv64"}):
            done = run.when_unpacked("true")
        # then
        self.assertEqual(done.code, 0, done.out)
        self.assertEqual(run.args(), expected_args(run, IN_ORDER))


class Cli(unittest.TestCase):
    def test_strict_takes_only_true_or_false(self):
        for value in (None, "yes"):
            with self.subTest(strict=value):
                # given
                run = setup(self, IN_ORDER)
                # when
                code = run.when_called_with_strict(value)
                # then
                self.assertEqual(code, 2)
                self.assertFalse(run.args_file.exists())


# ---------- setup: release archives and a runner ----------

Done = namedtuple("Done", "code out")


class Run:
    def __init__(self, test, tmp):
        self.test = test
        self.fetched, self.out, self.args_file = tmp / "fetched", tmp / "bins", tmp / "bin-args.txt"
        self.fetched.mkdir()

    def argv(self, *strict):
        return ["--fetched", self.fetched, "--out", self.out, "--tag", TAG, "--args-file", self.args_file, *strict]

    def when_unpacked(self, strict):
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            code = unpack_bins.main([str(a) for a in self.argv("--strict", strict)])
        return Done(code, buffer.getvalue())

    def when_called_with_strict(self, value):
        argv = [str(a) for a in self.argv(*(["--strict", value] if value else ["--strict"]))]
        with contextlib.redirect_stderr(io.StringIO()), self.test.assertRaises(SystemExit) as raised:
            unpack_bins.main(argv)
        return raised.exception.code

    def args(self):
        return self.args_file.read_text().splitlines()

    def binary(self, triple):
        return self.out / triple / f"meshtown-cli-{VERSION}-{triple}" / "meshtown-cli"


def setup(test, triples, extra=()):
    """A `fetched` directory with a release archive for each triple: <name>.tar.gz holding <name>/meshtown-cli."""
    run = Run(test, tmpdir(test))
    for triple in [*triples, *extra]:
        base = f"meshtown-cli-{VERSION}-{triple}"
        content = f"binary of {triple}".encode()
        with tarfile.open(run.fetched / f"{base}.tar.gz", "w:gz") as tar:
            info = tarfile.TarInfo(f"{base}/meshtown-cli")
            info.size = len(content)
            info.mode = 0o755
            tar.addfile(info, io.BytesIO(content))
    return run


def expected_args(run, triples):
    """The argument file lines: `--bin` then `triple=path` for every triple, in the given order."""
    return [line for t in triples for line in ("--bin", f"{t}={run.binary(t)}")]


if __name__ == "__main__":
    unittest.main()
