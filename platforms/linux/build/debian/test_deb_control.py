"""Tests of deb.py: the control file, reproducibility, refusals and the command line.

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/debian -t platforms/linux/build/debian
"""

import hashlib
import unittest
from pathlib import Path

import deb
from deb_support import (
    FILES,
    Opened,
    SCRIPTS,
    SCRIPT_NAMES,
    VERSION,
    ar_member,
    control_fields,
    prepared,
    run_cli,
    setup,
    source_date_epoch,
    tmpdir,
)


class Control(unittest.TestCase):
    def test_carries_the_package_fields(self):
        # given
        built = setup(self, arch="arm64")
        # when
        fields = control_fields(built.control_text())
        # then
        self.assertEqual(fields["Package"], "meshtown")
        self.assertEqual(fields["Version"], VERSION)
        self.assertEqual(fields["Architecture"], "arm64")
        self.assertEqual(fields["Maintainer"], deb.MAINTAINER)
        self.assertEqual(fields["Section"], "net")
        self.assertEqual(fields["Priority"], "optional")
        self.assertEqual(fields["Homepage"], "https://github.com/mesh-town/meshtown-dist")

    def test_has_no_dependencies(self):
        # given
        built = setup(self)
        # when
        fields = control_fields(built.control_text())
        # then
        self.assertEqual([k for k in fields if "Depends" in k], [], "the binary is static")

    def test_installed_size_is_in_kibibytes_of_the_data(self):
        # given
        built = setup(self, binary_bytes=b"\0" * 10240)
        # when
        size = int(control_fields(built.control_text())["Installed-Size"])
        # then
        self.assertGreaterEqual(size, 10, "at least the binary")
        self.assertLess(size, 20, "not counted in bytes")

    def test_description_is_a_summary_and_one_continuation_line(self):
        # given
        built = setup(self)
        # when
        lines = built.control_text().split("\n")
        # then
        at = next(i for i, line in enumerate(lines) if line.startswith("Description: "))
        self.assertGreater(len(lines[at]), len("Description: "), "summary on the first line")
        self.assertTrue(lines[at + 1].startswith(" ") and lines[at + 1].strip(), "continuation line")

    def test_ends_with_exactly_one_newline(self):
        # given
        built = setup(self)
        # when
        text = built.control_text()
        # then
        self.assertTrue(text.endswith("\n") and not text.endswith("\n\n"), repr(text[-20:]))

    def test_read_control_returns_the_same_text(self):
        # given
        built = setup(self)
        # when
        text = deb.read_control(built.path)
        # then
        self.assertEqual(text, built.control_text())
        self.assertTrue(text.startswith("Package: meshtown\n"), text[:40])


class ControlArchive(unittest.TestCase):
    def test_holds_control_md5sums_and_the_three_scripts_only(self):
        # given
        built = setup(self)
        # when
        names = {e.name.rstrip("/") for e in built.control_entries}
        # then
        self.assertEqual(names, {".", "./control", "./md5sums", "./postinst", "./prerm", "./postrm"})

    def test_has_no_conffiles_member(self):
        # given
        built = setup(self)
        # when
        names = [e.name for e in built.control_entries]
        # then
        self.assertNotIn("./conffiles", names)

    def test_scripts_are_executable_posix_shell_scripts_copied_from_the_source(self):
        # given
        built = setup(self)
        # when
        modes = {e.name: e.mode for e in built.control_entries}
        # then
        for name in SCRIPT_NAMES:
            body = built.control_file(name)
            self.assertEqual(modes[f"./{name}"], 0o755, name)
            self.assertTrue(body.startswith(b"#!/bin/sh\n"), name)
            self.assertEqual(body, (SCRIPTS / name).read_bytes(), name)

    def test_md5sums_matches_every_data_file(self):
        # given
        built = setup(self, with_license=True)
        # when
        sums = {}
        for line in built.control_file("md5sums").decode().splitlines():
            digest, path = line.split("  ", 1)
            sums[path] = digest
        # then
        self.assertEqual(set(sums), set(FILES), "one line per file, path without ./")
        for path in FILES:
            self.assertEqual(sums[path], hashlib.md5(built.data_file(path)).hexdigest(), path)


class Reproducibility(unittest.TestCase):
    def test_same_input_and_source_date_epoch_give_equal_bytes(self):
        # given
        with source_date_epoch("1700000000"):
            first = setup(self)
            # when
            second = setup(self)
        # then
        self.assertEqual(first.raw, second.raw)

    def test_same_input_without_source_date_epoch_gives_equal_bytes(self):
        # given
        with source_date_epoch(None):
            first = setup(self)
            # when
            second = setup(self)
        # then
        self.assertEqual(first.raw, second.raw)

    def test_source_date_epoch_is_the_time_of_every_tar_entry(self):
        # given
        with source_date_epoch("1700000000"):
            # when
            built = setup(self)
        # then
        for entry in built.control_entries + built.data_entries:
            self.assertEqual(entry.mtime, 1700000000, entry.name)

    def test_another_source_date_epoch_changes_the_bytes(self):
        # given
        with source_date_epoch("1700000000"):
            first = setup(self)
        with source_date_epoch("1700000001"):
            # when
            second = setup(self)
        # then
        self.assertNotEqual(first.raw, second.raw)


class Refusals(unittest.TestCase):
    def test_foreign_architecture(self):
        # given
        inputs = prepared(self)
        # when / then
        for arch in ("i386", "x86_64", "aarch64", "armhf", ""):
            with self.subTest(arch=arch):
                self.assert_refuses(inputs, arch=arch)

    def test_version_that_is_not_x_y_z(self):
        # given
        inputs = prepared(self)
        # when / then
        for version in ("0.3", "0.3.1.4", "0.3.1-1", "v0.3.1", "0.3.x", "", "1..2"):
            with self.subTest(version=version):
                self.assert_refuses(inputs, version=version)

    def test_missing_binary(self):
        # given
        inputs = prepared(self)
        # when / then
        self.assert_refuses(inputs, binary=inputs.tmp / "absent" / "meshtown")

    def test_missing_keyring(self):
        # given
        inputs = prepared(self)
        # when / then
        self.assert_refuses(inputs, keyring=inputs.tmp / "absent.gpg")

    def test_accepts_both_supported_architectures(self):
        # given
        inputs = prepared(self)
        # when
        for arch in sorted(deb.ARCHES):
            path = deb.build_deb(inputs.binary, VERSION, arch, inputs.out, keyring=inputs.keyring)
            # then
            self.assertTrue(path.is_file(), arch)
        self.assertEqual(sorted(deb.ARCHES), ["amd64", "arm64"])

    def assert_refuses(self, inputs, **override):
        args = {"binary": inputs.binary, "version": VERSION, "arch": "amd64", "out_dir": inputs.out,
                "keyring": inputs.keyring}
        args.update(override)
        with self.assertRaises(ValueError):
            deb.build_deb(**args)
        self.assertEqual(list(inputs.out.glob("*.deb")), [], "no package left behind")


class ReadControl(unittest.TestCase):
    def test_text_file_is_not_a_package(self):
        # given
        path = tmpdir(self) / "notes.deb"
        path.write_text("just some text, long enough to be more than a magic number\n")
        # when / then
        with self.assertRaises(ValueError):
            deb.read_control(path)

    def test_ar_without_control_archive_is_refused(self):
        # given
        path = tmpdir(self) / "half.deb"
        path.write_bytes(b"!<arch>\n" + ar_member("debian-binary", b"2.0\n"))
        # when / then
        with self.assertRaises(ValueError):
            deb.read_control(path)

    def test_empty_file_is_refused(self):
        # given
        path = tmpdir(self) / "empty.deb"
        path.write_bytes(b"")
        # when / then
        with self.assertRaises(ValueError):
            deb.read_control(path)


class CommandLine(unittest.TestCase):
    def test_prints_the_path_of_the_package_it_wrote(self):
        # given
        inputs = prepared(self)
        # when
        result = run_cli(inputs, "--arch", "arm64", "--version", VERSION)
        # then
        self.assertEqual(result.returncode, 0, result.stderr)
        path = Path(result.stdout.strip())
        self.assertEqual(path, inputs.out / "meshtown_0.3.47_arm64.deb")
        self.assertTrue(path.is_file())

    def test_license_flag_adds_the_copyright_file(self):
        # given
        inputs = prepared(self)
        # when
        result = run_cli(inputs, "--arch", "amd64", "--version", VERSION, "--license", str(inputs.license))
        # then
        self.assertEqual(result.returncode, 0, result.stderr)
        built = Opened(Path(result.stdout.strip()))
        self.assertIn("usr/share/doc/meshtown/copyright", built.data_file_modes())

    def test_bad_architecture_is_code_2_with_a_message_on_stderr(self):
        # given
        inputs = prepared(self)
        # when
        result = run_cli(inputs, "--arch", "i386", "--version", VERSION)
        # then
        self.assertEqual(result.returncode, 2)
        self.assertNotEqual(result.stderr.strip(), "")
        self.assertEqual(result.stdout, "")
        self.assertEqual(list(inputs.out.glob("*.deb")), [])

    def test_missing_binary_is_code_2(self):
        # given
        inputs = prepared(self)
        inputs.binary.unlink()
        # when
        result = run_cli(inputs, "--arch", "amd64", "--version", VERSION)
        # then
        self.assertEqual(result.returncode, 2)
        self.assertNotEqual(result.stderr.strip(), "")


if __name__ == "__main__":
    unittest.main()
