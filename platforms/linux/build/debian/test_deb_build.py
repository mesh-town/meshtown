"""Tests of deb.py: the ar container, the inner archives, the data archive and the apt source file.

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/debian -t platforms/linux/build/debian
"""

import gzip
import os
import unittest

from deb_support import FILES, HERE, KEYRING_BYTES, LICENSE_TEXT, deb822, setup


class ArContainer(unittest.TestCase):
    def test_starts_with_the_ar_magic(self):
        # given
        built = setup(self)
        # when
        magic = built.raw[:8]
        # then
        self.assertEqual(magic, b"!<arch>\n")

    def test_members_are_debian_binary_control_data_in_this_order(self):
        # given
        built = setup(self)
        # when
        names = [m.name for m in built.members]
        # then
        self.assertEqual(names, ["debian-binary", "control.tar.gz", "data.tar.gz"])

    def test_debian_binary_says_2_0(self):
        # given
        built = setup(self)
        # when
        body = built.member("debian-binary").body
        # then
        self.assertEqual(body, b"2.0\n")

    def test_every_header_is_60_bytes_owned_by_root_with_mode_100644(self):
        # given
        built = setup(self)
        # when
        headers = [m.header for m in built.members]
        # then
        for header in headers:
            self.assertEqual(len(header), 60)
            self.assertEqual(header[28:34].strip(), b"0", "uid")
            self.assertEqual(header[34:40].strip(), b"0", "gid")
            self.assertEqual(header[40:48].strip(), b"100644", "mode")
            self.assertEqual(header[58:60], b"`\n", "terminator")

    def test_member_names_have_no_dot_slash_and_no_trailing_slash(self):
        # given
        built = setup(self)
        # when
        raw_names = [m.header[:16] for m in built.members]
        # then
        for raw in raw_names:
            text = raw.decode().rstrip(" ")
            self.assertFalse(text.startswith("./") or text.endswith("/"), repr(raw))

    def test_odd_sized_member_is_padded_with_a_newline(self):
        # given: binaries of different lengths give gzip members of both parities
        seen_odd = False
        for length in range(1, 30):
            built = setup(self, binary_bytes=b"x" * length)
            # when
            seen_odd = seen_odd or any(m.size % 2 for m in built.members)
            # then: Opened.parse rejects a missing pad, the length adds up to the file
            expected = 8 + sum(60 + m.size + m.size % 2 for m in built.members)
            self.assertEqual(len(built.raw), expected)
        self.assertTrue(seen_odd, "no odd-sized member among the samples: widen the range")

    def test_file_name_is_package_version_arch(self):
        # given / when
        amd = setup(self, arch="amd64")
        arm = setup(self, arch="arm64", version="1.20.3")
        # then
        self.assertEqual(amd.path.name, "meshtown_0.3.47_amd64.deb")
        self.assertEqual(arm.path.name, "meshtown_1.20.3_arm64.deb")


class InnerArchives(unittest.TestCase):
    def test_are_gnu_tar_in_gzip_with_zero_gzip_mtime(self):
        # given
        built = setup(self)
        # when
        members = [built.member("control.tar.gz").body, built.member("data.tar.gz").body]
        # then
        for body in members:
            self.assertEqual(body[:2], b"\x1f\x8b", "gzip magic")
            self.assertEqual(body[4:8], b"\0\0\0\0", "gzip mtime")
            self.assertEqual(gzip.decompress(body)[257:265], b"ustar  \0", "GNU tar magic")

    def test_every_entry_is_owned_by_root_and_named_with_dot_slash(self):
        # given
        built = setup(self)
        # when
        entries = built.control_entries + built.data_entries
        # then
        for entry in entries:
            self.assertEqual((entry.uid, entry.gid, entry.uname, entry.gname), (0, 0, "root", "root"), entry.name)
        for raw in (built.control_raw_names + built.data_raw_names):
            self.assertTrue(raw.startswith("./"), raw)

    def test_both_archives_start_with_the_dot_slash_directory(self):
        # given
        built = setup(self)
        # when
        first_names = [built.control_raw_names[0], built.data_raw_names[0]]
        # then
        self.assertEqual(first_names, ["./", "./"])


class DataArchive(unittest.TestCase):
    def test_holds_exactly_five_files_with_the_documented_modes(self):
        # given
        built = setup(self, with_license=True)
        # when
        files = built.data_file_modes()
        # then
        self.assertEqual(files, FILES)

    def test_has_no_copyright_without_a_license_file(self):
        # given
        built = setup(self, with_license=False)
        # when
        files = built.data_file_modes()
        # then
        self.assertEqual(files, {k: v for k, v in FILES.items() if k != "usr/share/doc/meshtown/copyright"})

    def test_lists_the_root_and_every_parent_directory_with_mode_755(self):
        # given
        built = setup(self)
        # when
        dirs = built.data_dir_modes()
        # then
        parents = {"."}
        for path in FILES:
            parts = path.split("/")[:-1]
            parents.update("/".join(parts[:i]) for i in range(1, len(parts) + 1))
        self.assertEqual(set(dirs), parents)
        self.assertEqual({mode for mode in dirs.values()}, {0o755})

    def test_directory_precedes_its_content(self):
        # given
        built = setup(self)
        # when
        seen = set()
        for entry in built.data_entries:
            name = entry.name.rstrip("/")
            parent = os.path.dirname(name) if name != "." else None
            # then
            if parent is not None:
                self.assertIn(parent if parent else ".", seen, f"{name} comes before its directory")
            seen.add(name)

    def test_binary_and_keyring_are_carried_byte_for_byte(self):
        # given
        built = setup(self, binary_bytes=b"\x7fELF-fake" * 7)
        # when
        binary = built.data_file("usr/bin/meshtown")
        keyring = built.data_file("usr/share/keyrings/meshtown.gpg")
        # then
        self.assertEqual(binary, b"\x7fELF-fake" * 7)
        self.assertEqual(keyring, KEYRING_BYTES)

    def test_copyright_is_the_given_license_file(self):
        # given
        built = setup(self, with_license=True)
        # when
        text = built.data_file("usr/share/doc/meshtown/copyright")
        # then
        self.assertEqual(text, LICENSE_TEXT)

    def test_systemd_unit_runs_the_node_from_etc_meshtown(self):
        # given
        built = setup(self)
        # when
        unit = built.data_file("usr/lib/systemd/system/meshtown.service").decode()
        # then
        self.assertIn("ExecStart=/usr/bin/meshtown run --config /etc/meshtown/config.json\n", unit)
        self.assertIn("WantedBy=multi-user.target", unit)
        self.assertEqual(unit, (HERE / "meshtown.service").read_text(), "shipped verbatim from the source tree")


class AptSource(unittest.TestCase):
    def test_arm64_package_pins_arm64_and_the_keyring(self):
        # given
        built = setup(self, arch="arm64")
        # when
        source = deb822(built.data_file("etc/apt/sources.list.d/meshtown.sources").decode())
        # then
        self.assertEqual(source["Architectures"], "arm64")
        self.assertEqual(source["Signed-By"], "/usr/share/keyrings/meshtown.gpg")

    def test_amd64_package_pins_amd64(self):
        # given
        built = setup(self, arch="amd64")
        # when
        source = deb822(built.data_file("etc/apt/sources.list.d/meshtown.sources").decode())
        # then
        self.assertEqual(source["Architectures"], "amd64")

    def test_points_at_the_stable_main_repository_of_the_pages_site(self):
        # given
        built = setup(self)
        # when
        source = deb822(built.data_file("etc/apt/sources.list.d/meshtown.sources").decode())
        # then
        self.assertEqual(source["Types"], "deb")
        self.assertEqual(source["URIs"], "https://mesh-town.github.io/meshtown-dist/linux/debian")
        self.assertEqual(source["Suites"], "stable")
        self.assertEqual(source["Components"], "main")

    def test_no_placeholder_is_left(self):
        # given
        built = setup(self)
        # when
        text = built.data_file("etc/apt/sources.list.d/meshtown.sources").decode()
        # then
        self.assertNotIn("@", text)


if __name__ == "__main__":
    unittest.main()
