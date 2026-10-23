"""Tests of ipk.py: the .ipk package and the opkg index (no external tools, runs anywhere).

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/openwrt/feed -t platforms/linux/build/openwrt/feed
"""

import gzip
import hashlib
import unittest
from pathlib import Path

import ipk
from feed_fixtures import (
    OPENWRT_DIR,
    PACKAGE_FILES,
    Unpacked,
    build_test_ipk,
    control_fields,
    tmpdir,
)


class OuterArchive(unittest.TestCase):
    def test_members_are_debian_binary_data_control_in_this_order(self):
        # given
        built = setup(self)
        # when
        names = [name for name, _, _ in Unpacked(built.path).outer]
        # then
        self.assertEqual(names, ["./debian-binary", "./data.tar.gz", "./control.tar.gz"])

    def test_debian_binary_says_2_0(self):
        # given
        built = setup(self)
        # when
        members = {name: data for name, _, data in Unpacked(built.path).outer}
        # then
        self.assertEqual(members["./debian-binary"], b"2.0\n")

    def test_file_name_carries_package_version_and_label(self):
        # given
        built = setup(self, label="arm_cortex-a7", version="0.2.7-r3", machine="arm")
        # when
        name = built.path.name
        # then
        self.assertEqual(name, "meshtown_0.2.7-r3_arm_cortex-a7.ipk")


class DataArchive(unittest.TestCase):
    def test_holds_exactly_the_four_package_files(self):
        # given
        built = setup(self)
        # when
        files = Unpacked(built.path).data_files()
        # then
        self.assertEqual(sorted(files), sorted(PACKAGE_FILES))

    def test_files_have_the_documented_modes(self):
        # given
        built = setup(self)
        # when
        files = Unpacked(built.path).data_files()
        # then
        modes = {name: info.mode & 0o7777 for name, (info, _) in files.items()}
        self.assertEqual(modes, PACKAGE_FILES)

    def test_every_member_has_uid_and_gid_0(self):
        # given: the stage is owned by whoever runs the tests, not by root
        built = setup(self)
        # when
        owners = [(name, info.uid, info.gid) for name, info, _ in Unpacked(built.path).data]
        # then
        self.assertEqual([o for o in owners if o[1:] != (0, 0)], [], "uid and gid must be 0 for every member")

    def test_keep_list_names_the_node_directory(self):
        # given
        built = setup(self)
        # when
        _, data = Unpacked(built.path).data_files()["lib/upgrade/keep.d/meshtown"]
        # then
        self.assertEqual(data, b"/etc/meshtown/\n")

    def test_binary_and_service_files_are_the_sources_unchanged(self):
        # given
        built = setup(self)
        # when
        files = Unpacked(built.path).data_files()
        # then
        self.assertEqual(files["usr/bin/meshtown"][1], built.binary.read_bytes())
        self.assertEqual(files["etc/init.d/meshtown"][1], (OPENWRT_DIR / "init.d-meshtown").read_bytes())
        self.assertNotIn("etc/sysctl.d/90-meshtown.conf", files)


class ControlArchive(unittest.TestCase):
    def test_identifies_package_architecture_and_version(self):
        # given
        built = setup(self, label="arm_cortex-a7", version="0.2.7-r3", machine="arm")
        # when
        fields = control_fields(Unpacked(built.path).control_text())
        # then
        self.assertEqual(fields["Package"], "meshtown")
        self.assertEqual(fields["Architecture"], "arm_cortex-a7")
        self.assertEqual(fields["Version"], "0.2.7-r3")

    def test_states_licence_maintainer_and_section(self):
        # given
        built = setup(self)
        # when
        fields = control_fields(Unpacked(built.path).control_text())
        # then
        self.assertEqual(fields["License"], "AGPL-3.0-only")
        self.assertEqual(fields["Maintainer"], "Mesh Town <340663027+mesh-town@users.noreply.github.com>")
        self.assertEqual(fields["Section"], "net")
        self.assertTrue(fields["Description"], "the index needs a Description to insert its fields before")

    def test_has_no_dependencies_because_the_binary_is_static(self):
        # given
        built = setup(self)
        # when
        fields = control_fields(Unpacked(built.path).control_text())
        # then
        self.assertNotIn("Depends", fields)

    def test_declares_no_conffiles(self):
        # given: a conffiles member would make opkg complain about a file the package does not ship
        built = setup(self)
        # when
        members = Unpacked(built.path).control_files()
        # then
        self.assertNotIn("conffiles", members)
        self.assertEqual(sorted(members), ["control", "postinst", "prerm"])

    def test_install_scripts_are_executable_and_use_the_sdk_helpers(self):
        # given
        built = setup(self)
        # when
        members = Unpacked(built.path).control_files()
        # then
        for script, helper in (("postinst", b"default_postinst"), ("prerm", b"default_prerm")):
            info, data = members[script]
            self.assertTrue(info.mode & 0o100, f"{script} must be executable")
            self.assertTrue(data.startswith(b"#!/bin/sh"), f"{script} must start with a shebang")
            self.assertIn(helper, data)


class ScriptsDirectory(unittest.TestCase):
    def test_control_scripts_come_from_the_given_directory(self):
        # given
        tmp = tmpdir(self)
        scripts = tmp / "scripts"
        scripts.mkdir()
        (scripts / "postinst").write_text("#!/bin/sh\necho custom-postinst\n")
        (scripts / "prerm").write_text("#!/bin/sh\necho custom-prerm\n")
        stage = tmp / "stage"
        (stage / "opt").mkdir(parents=True)
        (stage / "opt" / "f").write_text("x")
        # when
        path = ipk.build_ipk(stage, "mips-3.4", "0.1.0-r1", tmp / "out", scripts)
        # then
        control = Unpacked(path).control_files()
        self.assertEqual(control["postinst"][1], b"#!/bin/sh\necho custom-postinst\n")
        self.assertEqual(control["prerm"][1], b"#!/bin/sh\necho custom-prerm\n")


class PackagesIndex(unittest.TestCase):
    def test_each_entry_matches_its_file_name_size_and_hash(self):
        # given: two versions in one directory
        out = indexed_directory(self, ["0.2.7-r1", "0.2.7-r2"])
        # when
        entries = parse_packages(out / "Packages")
        # then
        self.assertEqual(len(entries), 2)
        for fields in entries:
            package = out / fields["Filename"]
            self.assertEqual(int(fields["Size"]), package.stat().st_size)
            self.assertEqual(fields["SHA256sum"], hashlib.sha256(package.read_bytes()).hexdigest())
        self.assertEqual(sorted(f["Version"] for f in entries), ["0.2.7-r1", "0.2.7-r2"])

    def test_added_fields_come_before_description(self):
        # given
        out = indexed_directory(self, ["0.2.7-r1"])
        # when
        order = list(parse_packages(out / "Packages")[0])
        # then
        for field in ("Filename", "Size", "SHA256sum"):
            self.assertLess(order.index(field), order.index("Description"), f"{field} must precede Description")

    def test_compressed_copy_unpacks_to_the_same_index(self):
        # given
        out = indexed_directory(self, ["0.2.7-r1", "0.2.7-r2"])
        # when
        unpacked = gzip.decompress((out / "Packages.gz").read_bytes())
        # then
        self.assertEqual(unpacked, (out / "Packages").read_bytes())

    def test_returns_the_path_of_the_index(self):
        # given
        tmp = tmpdir(self)
        out = tmp / "opkg"
        build_test_ipk(tmp / "a", out_dir=out)
        # when
        result = ipk.write_index(out)
        # then
        # write_index returns a Path: the path of Packages
        self.assertEqual(Path(result), out / "Packages")

    def test_length_never_hits_the_usign_bug_window(self):
        # given: sizes sweep across a whole 128-byte period, for two parities of Installed-Size digits
        # when / then
        for pad in (0, 20_000):
            for k in range(64):
                with self.subTest(pad=pad, k=k):
                    tmp = tmpdir(self)
                    build_test_ipk(tmp / "a", version="0.2.7-r" + "1" * (k + 1), pad=pad, out_dir=tmp / "out")
                    ipk.write_index(tmp / "out")
                    length = (tmp / "out" / "Packages").stat().st_size
                    self.assertNotIn((64 + length) % 128, (110, 111), f"Packages is {length} bytes")


class Reproducibility(unittest.TestCase):
    def test_same_input_gives_the_same_package_and_index_bytes(self):
        # given: two builds in different directories, the second stage touched later
        first = setup(self, machine="x86_64", label="x86_64")
        second = setup(self, machine="x86_64", label="x86_64", touch_stage=True)
        # when
        ipk.write_index(first.path.parent)
        ipk.write_index(second.path.parent)
        # then
        self.assertEqual(first.path.read_bytes(), second.path.read_bytes())
        self.assertEqual((first.path.parent / "Packages").read_bytes(), (second.path.parent / "Packages").read_bytes())


# ---------- setup and helpers ----------

def setup(test, label="aarch64_generic", version="0.2.7-r1", machine="aarch64", touch_stage=False):
    """A built .ipk in a directory of its own."""
    return build_test_ipk(tmpdir(test), label, version, machine, touch_stage=touch_stage)


def indexed_directory(test, versions):
    """A directory with one .ipk per version and its Packages index."""
    tmp = tmpdir(test)
    out = tmp / "opkg"
    for number, version in enumerate(versions):
        build_test_ipk(tmp / f"b{number}", version=version, out_dir=out)
    ipk.write_index(out)
    return out


def parse_packages(path):
    """Entries of a Packages file as ordered {field: value} dictionaries."""
    blocks = [b for b in Path(path).read_text().split("\n\n") if b.strip()]
    return [control_fields(block) for block in blocks]


if __name__ == "__main__":
    unittest.main()
