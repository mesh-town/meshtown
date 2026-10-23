"""Tests of apk.py: the .apk package and its signed index.

Building a package needs apk-tools and root (or fakeroot): without them those tests are skipped,
unless MESHTOWN_FEED_REQUIRE_TOOLS=1, which turns every skip into a failure. Checking the index
signature needs apk-tools only. The root check itself needs nothing.

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/openwrt/feed -t platforms/linux/build/openwrt/feed
"""

import re
import subprocess
import unittest
from collections import namedtuple
from pathlib import Path

import apk
import build_feed
from feed_fixtures import (
    PACKAGE_FILES,
    apk_or_skip,
    bare_apk,
    fake_elf,
    make_keys,
    root_or_skip,
    tmpdir,
    tools_or_skip,
)

LIST_FILE = "lib/apk/packages/meshtown.list"
SCRIPTS = {"post-install", "post-upgrade", "pre-upgrade", "pre-deinstall"}


class RootCheck(unittest.TestCase):
    def test_ordinary_user_is_refused_with_a_hint_about_fakeroot(self):
        # given / when / then
        with self.assertRaises(PermissionError) as raised:
            apk.require_root(1000)
        self.assertIn("fakeroot", str(raised.exception))

    def test_root_is_accepted(self):
        # given / when
        result = apk.require_root(0)
        # then
        self.assertIsNone(result)


class PackageMetadata(unittest.TestCase):
    def test_architecture_name_and_version_come_from_the_call(self):
        # given
        dump = setup(self, label="aarch64_cortex-a53", version="0.2.7-r3").dump
        # when
        info = info_of(dump)
        # then
        self.assertEqual((info["name"], info["version"], info["arch"]), ("meshtown", "0.2.7-r3", "aarch64_cortex-a53"))

    def test_licence_maintainer_and_url_are_the_project_ones(self):
        # given
        dump = setup(self).dump
        # when
        info = info_of(dump)
        # then
        self.assertEqual(info["license"], "AGPL-3.0-only")
        self.assertEqual(info["maintainer"], "Mesh Town <340663027+mesh-town@users.noreply.github.com>")
        self.assertEqual(info["url"], "https://github.com/mesh-town/meshtown")

    def test_has_the_four_maintainer_scripts_and_no_others(self):
        # given
        dump = setup(self).dump
        # when
        scripts = scripts_of(dump)
        # then
        self.assertEqual(set(scripts), SCRIPTS)

    def test_upgrade_scripts_set_the_upgrade_flag_and_pre_upgrade_stops_the_service(self):
        # given
        scripts = scripts_of(setup(self).dump)
        # when / then
        self.assertIn("PKG_UPGRADE=1", scripts["pre-upgrade"])
        self.assertIn("default_prerm", scripts["pre-upgrade"])
        self.assertIn("PKG_UPGRADE=1", scripts["post-upgrade"])
        self.assertIn("default_postinst", scripts["post-install"])
        self.assertIn("default_prerm", scripts["pre-deinstall"])
        self.assertNotIn("PKG_UPGRADE", scripts["pre-deinstall"])

    def test_declares_no_conffiles(self):
        # given
        dump = setup(self).dump
        # when / then
        self.assertNotIn("conffiles", dump)

    def test_preserves_no_extended_attributes(self):
        # given: attributes make apk on the router print "failed to preserve ... xattrs"
        dump = setup(self).dump
        # when / then
        self.assertNotIn("xattrs", dump)

    def test_ships_the_package_files_and_the_file_list(self):
        # given
        dump = setup(self).dump
        # when
        files = files_of(dump)
        # then
        self.assertEqual(files, set(PACKAGE_FILES) | {LIST_FILE})


class PackageOwnership(unittest.TestCase):
    def test_every_entry_is_owned_by_root_and_group_root(self):
        # given
        dump = setup(self).dump
        # when
        owners = set(re.findall(r"^\s+(?:user|group): (\S+)$", dump, re.MULTILINE))
        # then
        self.assertEqual(owners, {"root"})
        self.assertGreater(len(re.findall(r"^\s+group: ", dump, re.MULTILINE)), 5, "the dump must list the entries")


class IndexSignature(unittest.TestCase):
    def test_accepts_the_index_signed_with_the_matching_key(self):
        # given
        index = signed_index(self)
        # when
        ok = apk.verify_apk_index(index.adb, index.pubdir, index.apk)
        # then
        self.assertIs(ok, True)

    def test_rejects_an_index_with_a_damaged_byte(self):
        index = signed_index(self)
        for where in (0, 30, index.adb.stat().st_size // 2, -20, -40):
            with self.subTest(position=where):
                # given
                damaged = index.damaged_copy(where)
                # when
                ok = apk.verify_apk_index(damaged, index.pubdir, index.apk)
                # then
                self.assertIs(ok, False)

    def test_rejects_an_index_checked_against_another_key(self):
        # given
        index = signed_index(self)
        _, _, other_pubdir = make_keys(tmpdir(self) / "other")
        # when
        ok = apk.verify_apk_index(index.adb, other_pubdir, index.apk)
        # then
        self.assertIs(ok, False)

    def test_returns_the_path_of_the_index_it_wrote(self):
        # given
        apk_bin = apk_or_skip()
        _, apk_sec, _ = make_keys(tmpdir(self) / "keys")
        out = tmpdir(self)
        bare_apk(apk_bin, out)
        # when
        result = apk.write_index(out, apk_bin, apk_sec)
        # then
        # write_index returns a Path: the path of packages.adb
        self.assertEqual(Path(result), out / "packages.adb")


# ---------- setup and helpers ----------

Built = namedtuple("Built", "path dump")


def setup(test, label="aarch64_generic", version="0.2.7-r1"):
    """A real .apk built from the stage tree (needs apk-tools and root), and its adbdump."""
    _, apk_bin = tools_or_skip()
    root_or_skip()
    tmp = tmpdir(test)
    stage = tmp / "stage"
    build_feed.stage_tree(stage, fake_elf(tmp / "meshtown-bin", "aarch64"), "apk")
    out = tmp / "out"
    out.mkdir()
    path = apk.build_apk(stage, label, version, out, apk_bin)
    return Built(path, adbdump(apk_bin, path))


def signed_index(test):
    """packages.adb signed by a fresh key, over a bare package (needs apk-tools, not root)."""
    apk_bin = apk_or_skip()
    tmp = tmpdir(test)
    _, apk_sec, pubdir = make_keys(tmp / "keys")
    out = tmp / "out"
    out.mkdir()
    bare_apk(apk_bin, out)
    apk.write_index(out, apk_bin, apk_sec)
    return SignedIndex(out / "packages.adb", pubdir, apk_bin, tmp)


class SignedIndex:
    def __init__(self, adb, pubdir, apk_bin, tmp):
        self.adb, self.pubdir, self.apk, self.tmp = adb, pubdir, apk_bin, tmp

    def damaged_copy(self, position):
        data = bytearray(self.adb.read_bytes())
        data[position] ^= 0x01
        copy = self.tmp / f"damaged-{position}.adb"
        copy.write_bytes(bytes(data))
        return copy


def adbdump(apk_bin, path):
    done = subprocess.run([str(apk_bin), "adbdump", str(path)], check=True, capture_output=True, text=True)
    return done.stdout


def info_of(dump):
    """The two-space indented `key: value` lines of the info block."""
    return dict(re.findall(r"^  ([a-z-]+): (.*)$", dump.split("\npaths:", 1)[0], re.MULTILINE))


def scripts_of(dump):
    """{script name: script text} from the scripts block of an adbdump."""
    scripts, name = {}, None
    for line in dump.split("\nscripts:\n", 1)[1].splitlines():
        header = re.match(r"^  ([a-z-]+): \|$", line)
        if line and not line.startswith(" "):
            break
        if header:
            name = header.group(1)
            scripts[name] = ""
        elif name:
            scripts[name] += line[4:] + "\n"
    return scripts


def files_of(dump):
    """Paths of the regular files in an adbdump, without a leading slash."""
    files, directory = set(), ""
    for line in dump.splitlines():
        entry_dir = re.match(r"^  - name: (.+)$", line)
        entry_file = re.match(r"^      - name: (.+)$", line)
        directory = entry_dir.group(1) if entry_dir else directory
        if entry_file:
            files.add(f"{directory}/{entry_file.group(1)}")
    return files


if __name__ == "__main__":
    unittest.main()
