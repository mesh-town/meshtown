"""Tests of the build_feed.py command line: layout, tools, exit codes and reproducibility.

Refusals (exit 2) and tool lookup run anywhere with stub tools. Building a signed feed needs usign and
openssl; adding the apk half needs apk-tools and root (or fakeroot). Without them those tests are
skipped, unless MESHTOWN_FEED_REQUIRE_TOOLS=1 turns a skip into a failure.

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/openwrt/feed -t platforms/linux/build/openwrt/feed
"""

import os
import unittest
from collections import namedtuple
from pathlib import Path

import keys
from feed_fixtures import (
    AARCH64,
    X86_64,
    Unpacked,
    apk_or_skip,
    dummy_keys,
    fake_elf,
    listing,
    make_keys,
    openssl_or_skip,
    root_or_skip,
    run_cli,
    stub_tool,
    tmpdir,
    usign_or_skip,
)

BASE = "https://feed.example.com/openwrt"
FIVE_LABELS = ["aarch64_cortex-a53", "aarch64_cortex-a72", "aarch64_cortex-a76", "aarch64_generic", "x86_64"]
SECRET_MARKERS = (b"PRIVATE " + b"KEY", b"secret " + b"key")
EPOCH = 1_700_000_000


class FeedLayout(unittest.TestCase):
    def test_every_label_whose_target_was_given_gets_a_signed_opkg_directory(self):
        # given
        feed = setup(self)
        # when
        done = feed.when_built("--only", "ipk", *feed.ephemeral())
        # then
        self.assertEqual(done.code, 0, done.err)
        self.assertEqual(listing(feed.out / "opkg"), sorted(FIVE_LABELS), "no arm label: no armv7 binary was given")
        for label in FIVE_LABELS:
            expected = {f"meshtown_0.2.7-r1_{label}.ipk", "Packages", "Packages.gz", "Packages.sig"}
            self.assertEqual(set(listing(feed.out / "opkg" / label)), expected, label)

    def test_root_holds_the_install_script_the_public_key_and_the_page_files(self):
        # given
        feed = setup(self)
        # when
        feed.when_built("--only", "ipk", *feed.ephemeral())
        # then
        for name in ("install.sh", "meshtown-usign.pub", "index.html", "README.md", ".nojekyll"):
            self.assertIn(name, listing(feed.out))

    def test_install_script_carries_the_base_url(self):
        # given
        feed = setup(self)
        # when
        feed.when_built("--only", "ipk", *feed.ephemeral())
        # then
        self.assertIn(BASE, (feed.out / "install.sh").read_text())
        self.assertNotIn("@BASE@", (feed.out / "install.sh").read_text())

    def test_index_signature_is_checked_by_the_published_key(self):
        # given
        feed = setup(self)
        feed.when_built("--only", "ipk", *feed.ephemeral())
        # when
        index = feed.out / "opkg" / "x86_64" / "Packages"
        ok = keys.verify_usign(index, feed.out / "meshtown-usign.pub", usign_or_skip())
        # then
        self.assertTrue(ok)

    def test_labels_narrow_the_build(self):
        # given
        feed = setup(self)
        # when
        done = feed.when_built("--only", "ipk", "--labels", "aarch64_generic", *feed.ephemeral())
        # then
        self.assertEqual(done.code, 0, done.err)
        self.assertEqual(listing(feed.out / "opkg"), ["aarch64_generic"])

    def test_prints_one_line_with_path_and_size_for_every_package(self):
        # given
        feed = setup(self)
        # when
        done = feed.when_built("--only", "ipk", "--labels", "aarch64_generic,x86_64", *feed.ephemeral())
        # then
        for label in ("aarch64_generic", "x86_64"):
            package = feed.out / "opkg" / label / f"meshtown_0.2.7-r1_{label}.ipk"
            lines = [line for line in done.out.splitlines() if package.name in line]
            self.assertEqual(len(lines), 1, done.out)
            self.assertIn(str(package.stat().st_size), lines[0])

    def test_the_secret_keys_are_not_published(self):
        # given: explicit keys, so that their text is known
        feed = setup(self)
        usign_sec, apk_sec, pubdir = make_keys(feed.tmp / "keys")
        # when
        done = feed.when_built("--only", "ipk", *feed.explicit(usign_sec, apk_sec, pubdir))
        # then
        self.assertEqual(done.code, 0, done.err)
        secret_body = usign_sec.read_text().splitlines()[1].encode()
        for path in sorted(p for p in feed.out.rglob("*") if p.is_file()):
            data = path.read_bytes()
            self.assertNotIn(secret_body, data, path.name)
            for marker in SECRET_MARKERS:
                self.assertNotIn(marker, data, path.name)


class FeedWithBothFormats(unittest.TestCase):
    def test_root_and_apk_directories_follow_the_published_layout(self):
        # given
        feed = setup(self)
        root_or_skip()
        # when
        done = feed.when_built(*feed.ephemeral(with_apk=True))
        # then
        self.assertEqual(done.code, 0, done.err)
        root = {"install.sh", "meshtown-usign.pub", "meshtown-apk.pem", "index.html", "README.md", ".nojekyll", "opkg", "apk"}
        self.assertEqual(set(listing(feed.out)), root)
        self.assertEqual(listing(feed.out / "apk"), sorted(FIVE_LABELS))
        for label in FIVE_LABELS:
            self.assertEqual(listing(feed.out / "apk" / label), ["meshtown-0.2.7-r1.apk", "packages.adb"], label)


class OnlyIpk(unittest.TestCase):
    def test_leaves_out_the_apk_directory_and_needs_no_apk_tool(self):
        # given
        feed = setup(self)
        # when
        done = feed.when_built("--only", "ipk", *feed.ephemeral(), env={"MESHTOWN_APK": "/nonexistent/apk"})
        # then
        self.assertEqual(done.code, 0, done.err)
        self.assertNotIn("apk", listing(feed.out))
        self.assertIn("opkg", listing(feed.out))


class Refusals(unittest.TestCase):
    def test_output_directory_that_is_not_empty(self):
        # given
        feed = setup(self)
        (feed.out).mkdir()
        (feed.out / "keep.txt").write_text("mine")
        # when
        done = feed.when_refused()
        # then
        self.assertEqual(done.code, 2, done.err)
        self.assertEqual(listing(feed.out), ["keep.txt"], "an existing directory must be left alone")

    def test_version_that_is_not_three_numbers(self):
        for version in ("0.2", "v0.2.7", "0.2.7.1", "0.2.x"):
            with self.subTest(version=version):
                # given
                feed = setup(self)
                # when
                done = feed.when_refused(version=version)
                # then
                self.assertEqual(done.code, 2, done.err)

    def test_package_release_that_is_not_a_positive_integer(self):
        for release in ("0", "-1", "one"):
            with self.subTest(release=release):
                # given
                feed = setup(self)
                # when
                done = feed.when_refused("--pkg-release", release)
                # then
                self.assertEqual(done.code, 2, done.err)

    def test_binary_of_another_machine_than_its_target(self):
        # given: an x86_64 file given for the aarch64 target
        feed = setup(self, machines={AARCH64: "x86_64", X86_64: "x86_64"})
        # when
        done = feed.when_refused()
        # then
        self.assertEqual(done.code, 2, done.err)

    def test_dynamically_linked_binary(self):
        # given
        feed = setup(self, static=False)
        # when
        done = feed.when_refused()
        # then
        self.assertEqual(done.code, 2, done.err)

    def test_missing_usign_tool(self):
        # given: no flag, no variable, an empty PATH
        feed = setup(self)
        empty = tmpdir(self)
        # when
        done = feed.when_built("--only", "ipk", *feed.dummy(with_usign=False), env={"PATH": str(empty)})
        # then
        self.assertEqual(done.code, 2, done.err)


class ToolLookup(unittest.TestCase):
    def test_flag_wins_over_the_variable_and_the_variable_over_the_path(self):
        # given: three stubs that record being called
        feed = setup(self)
        flagged, named, found = (recording_tool(feed.tmp / where) for where in ("flag", "var", "path"))
        # when
        feed.when_built("--only", "ipk", *feed.dummy(with_usign=False), "--usign", flagged, env={"MESHTOWN_USIGN": named})
        # then
        self.assertTrue(called(flagged), "--usign must be used")
        self.assertFalse(called(named), "the variable must lose to the flag")
        # when
        feed.out = feed.tmp / "second"
        feed.when_built("--only", "ipk", *feed.dummy(with_usign=False),
                        env={"MESHTOWN_USIGN": named, "PATH": f"{found.parent}:{os.environ['PATH']}"})
        # then
        self.assertTrue(called(named), "MESHTOWN_USIGN must be used")
        self.assertFalse(called(found), "the path must lose to the variable")


class WrongPublicKey(unittest.TestCase):
    def test_a_feed_not_matching_the_given_public_keys_fails_with_1(self):
        # given: signed by one pair, checked against the public half of another
        feed = setup(self)
        usign_sec, apk_sec, _ = make_keys(feed.tmp / "keys")
        _, _, other_pubdir = make_keys(feed.tmp / "other")
        # when
        done = feed.when_built("--only", "ipk", *feed.explicit(usign_sec, apk_sec, other_pubdir))
        # then
        self.assertEqual(done.code, 1, done.err)


class OrdinaryUserAskingForApk(unittest.TestCase):
    def test_fails_with_1_and_points_to_fakeroot(self):
        # given
        feed = setup(self)
        apk_or_skip()
        if os.geteuid() == 0:
            self.skipTest("already root")
        # when
        done = feed.when_built(*feed.ephemeral(with_apk=True))
        # then
        self.assertEqual(done.code, 1, done.err)
        self.assertIn("fakeroot", done.err)


class Reproducibility(unittest.TestCase):
    def test_same_input_gives_the_same_packages_and_index(self):
        # given
        first, second = setup(self), setup(self)
        # when
        for feed in (first, second):
            feed.when_built("--only", "ipk", "--labels", "aarch64_generic", *feed.ephemeral(), env={"SOURCE_DATE_EPOCH": EPOCH})
        # then
        for name in ("meshtown_0.2.7-r1_aarch64_generic.ipk", "Packages"):
            one = (first.out / "opkg" / "aarch64_generic" / name).read_bytes()
            other = (second.out / "opkg" / "aarch64_generic" / name).read_bytes()
            self.assertEqual(one, other, name)

    def test_file_times_inside_the_package_are_source_date_epoch(self):
        # given
        feed = setup(self)
        # when
        feed.when_built("--only", "ipk", "--labels", "x86_64", *feed.ephemeral(), env={"SOURCE_DATE_EPOCH": EPOCH})
        # then
        # "mtime of the files" is read as the member times of the package archives
        package = feed.out / "opkg" / "x86_64" / "meshtown_0.2.7-r1_x86_64.ipk"
        times = {info.mtime for _, info, _ in Unpacked(package).data if info.isfile()}
        self.assertEqual(times, {EPOCH})


# ---------- setup: inputs, arguments and a runner ----------

Done = namedtuple("Done", "code out err")


class Feed:
    def __init__(self, tmp, bins):
        self.tmp, self.out, self.bins = tmp, tmp / "feed", bins
        self._dummy_keys = None

    def when_built(self, *extra, version="0.2.7", env=None):
        args = ["--out", self.out, "--version", version, "--base-url", BASE, *self.bins, *extra]
        return Done(*run_cli(*args, env=env))

    def when_refused(self, *extra, version="0.2.7"):
        """A call that must be turned down before any tool is needed: stub tools, files that are no keys."""
        return self.when_built("--only", "ipk", *self.dummy(), *extra, version=version)

    def ephemeral(self, with_apk=False):
        """Arguments for a one-off key pair, the tools found on this host."""
        usign, _ = usign_or_skip(), openssl_or_skip()
        apk = ["--apk", apk_or_skip()] if with_apk else []
        return ["--ephemeral-keys", "--usign", usign, *apk]

    def explicit(self, usign_sec, apk_sec, pubdir):
        return ["--usign-key", usign_sec, "--apk-key", apk_sec, "--pubkeys", pubdir, "--usign", usign_or_skip()]

    def dummy(self, with_usign=True):
        """Key arguments that name files with no keys in them, and stub tools."""
        self._dummy_keys = self._dummy_keys or dummy_keys(self.tmp / "dummy")
        usign_sec, apk_sec, pubdir = self._dummy_keys
        stubs = ["--usign", stub_tool(self.tmp / "usign-stub")] if with_usign else []
        return ["--usign-key", usign_sec, "--apk-key", apk_sec, "--pubkeys", pubdir, *stubs]


def setup(test, machines=None, static=True):
    """Binaries for the aarch64 and x86_64 targets (`machines` overrides which machine a target's file is)."""
    tmp = tmpdir(test)
    machines = machines or {AARCH64: "aarch64", X86_64: "x86_64"}
    bins = []
    for triple, machine in machines.items():
        path = fake_elf(tmp / f"meshtown-{triple}", machine, static=static)
        bins += ["--bin", f"{triple}={path}"]
    return Feed(tmp, bins)


def recording_tool(directory):
    """An executable `usign` that appends a line to <it>.log when run and does nothing else."""
    directory.mkdir(parents=True)
    tool = directory / "usign"
    tool.write_text(f'#!/bin/sh\necho called >> "{tool}.log"\nexit 1\n')
    tool.chmod(0o755)
    return tool


def called(tool):
    return Path(f"{tool}.log").exists()


if __name__ == "__main__":
    unittest.main()
